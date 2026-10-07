use super::*;

// =======================================================================
// T210: check_only — GitHub APIチェックのみ同期、DLは行わない
// =======================================================================
#[tokio::test]
async fn check_only_does_not_download_payload() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
            .and(path(format!("/repos/{DEFAULT_OWNER}/{DEFAULT_REPO}/releases/latest")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": "v99.0.0",
                "html_url": "https://github.com/test-owner/test-repo/releases/tag/v99.0.0",
                "assets": [{
                    "name": format!("llmlb-{}.tar.gz", Platform::detect().unwrap().artifact().unwrap_or("linux-x86_64")),
                    "browser_download_url": format!("{}/download/portable.tar.gz", mock_server.uri()),
                }]
            })))
            .mount(&mock_server)
            .await;

    let tmp = tempfile::tempdir().expect("create temp dir");
    let manager = UpdateManager::new_with_data_dir_and_config(
        reqwest::Client::new(),
        InferenceGate::default(),
        ShutdownController::default(),
        tmp.path(),
        Some(mock_server.uri()),
    )
    .expect("create update manager");

    let state = manager.check_only(true).await.expect("check_only");

    // 並列実行するテストプロセスと競合しないよう、キャッシュは一時データディレクトリにだけ書く (#761)。
    assert!(
        manager.inner.cache_path.starts_with(tmp.path()),
        "cache must be written under the temp data dir: {}",
        manager.inner.cache_path.display()
    );
    assert!(
        manager.inner.cache_path.exists(),
        "check_only should save the cache"
    );

    // Should discover the update.
    match &state {
        UpdateState::Available {
            latest, payload, ..
        } => {
            assert_eq!(latest, "99.0.0");
            // check_only must NOT start downloading.
            assert_eq!(*payload, PayloadState::NotReady);
        }
        other => panic!("expected available, got {other:?}"),
    }
}

// =======================================================================
// T211: download_background — バックグラウンドDL開始、進捗更新
// =======================================================================
#[tokio::test]
async fn download_background_transitions_to_downloading() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    // 実行ファイルを含む小さな tar.gz を返し、ダウンロードから展開まで成功させる。
    // 展開できない内容を返すと、準備失敗で payload が即座に `Error` になる (#760)。
    let binary_name = Platform::detect().unwrap().binary_name();
    let archive = tar_gz_with_files(&[(format!("llmlb-test/{binary_name}").as_str(), b"bin")]);
    let archive_len = archive.len().to_string();
    Mock::given(method("GET"))
        .and(path("/download/portable.tar.gz"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(archive)
                .insert_header("content-length", archive_len.as_str()),
        )
        .mount(&mock_server)
        .await;

    let (manager, _tmp) = test_manager_with_gate(InferenceGate::default());

    // Pre-seed available state with a portable asset URL pointing to mock.
    {
        let mut st = manager.inner.state.write().await;
        *st = UpdateState::Available {
            current: "4.5.0".to_string(),
            latest: "4.5.1".to_string(),
            release_url: "https://example.com/release".to_string(),
            portable_asset_url: Some(format!("{}/download/portable.tar.gz", mock_server.uri())),
            installer_asset_url: None,
            payload: PayloadState::NotReady,
            checked_at: Utc::now(),
        };
    }

    // Start background download.
    manager.download_background();

    // `Downloading { downloaded_bytes: None }` は適用プラン選定の前に書かれ、直後に `Error` へ
    // 落ちることがある。受信バイト数は選定を通過して実際に受信したときだけ記録されるので、
    // それを待ち条件にする (#754)。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let payload = match manager.state().await {
            UpdateState::Available { payload, .. } => payload,
            other => panic!("expected available, got {other:?}"),
        };
        match &payload {
            PayloadState::Downloading {
                downloaded_bytes: Some(n),
                ..
            } if *n > 0 => break,
            PayloadState::Ready { .. } => break,
            PayloadState::Error { message } => panic!("download failed: {message}"),
            _ => {}
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "download did not progress in time, last payload: {payload:?}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

// =======================================================================
// #760: ペイロード準備の失敗は payload を Error へ遷移させる
// =======================================================================

/// 各テスト固有のベースパスに置いた portable アセットで Available 状態を作る。
/// ベースパスを一意にするのは、別プロセスの要求がモックに届いても照合させないため。
async fn seed_available_with_portable_url(manager: &UpdateManager, url: String) {
    let mut st = manager.inner.state.write().await;
    *st = UpdateState::Available {
        current: "4.5.0".to_string(),
        latest: "4.5.1".to_string(),
        release_url: "https://example.com/release".to_string(),
        portable_asset_url: Some(url),
        installer_asset_url: None,
        payload: PayloadState::NotReady,
        checked_at: Utc::now(),
    };
}

/// `entries` のファイルを格納した tar.gz を作る。
fn tar_gz_with_files(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for (name, data) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append_data(&mut header, name, *data).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

/// `body` を返す portable アセットをモックし、そのURLを返す。
async fn mock_portable_asset(
    mock_server: &wiremock::MockServer,
    response: wiremock::ResponseTemplate,
) -> String {
    use wiremock::matchers::{method, path};
    use wiremock::Mock;

    let asset_path = format!("/{}/portable.tar.gz", uuid::Uuid::new_v4().simple());
    Mock::given(method("GET"))
        .and(path(asset_path.clone()))
        .respond_with(response)
        .mount(mock_server)
        .await;
    format!("{}{asset_path}", mock_server.uri())
}

/// `ensure_payload_ready` が失敗し、payload が `Error` に遷移していることを確かめる。
async fn assert_payload_error(
    manager: &UpdateManager,
    result: anyhow::Result<PayloadKind>,
    expected: &str,
) {
    let err = result.expect_err("payload preparation should fail");
    match manager.state().await {
        UpdateState::Available {
            payload: PayloadState::Error { message },
            ..
        } => {
            assert!(
                message.contains(expected),
                "error message {message:?} should contain {expected:?} (err: {err:#})"
            );
        }
        other => panic!("expected payload Error, got {other:?} (err: {err:#})"),
    }
}

#[tokio::test]
async fn ensure_payload_ready_http_error_sets_payload_error() {
    let mock_server = wiremock::MockServer::start().await;
    let url = mock_portable_asset(&mock_server, wiremock::ResponseTemplate::new(404)).await;
    let (manager, _tmp) = test_manager_with_gate(InferenceGate::default());
    seed_available_with_portable_url(&manager, url).await;

    let result = manager.ensure_payload_ready().await;

    assert_payload_error(&manager, result, "Failed to download update payload").await;
    // 利用者が原因を判断できるよう、根本原因も残す。
    let UpdateState::Available {
        payload: PayloadState::Error { message },
        ..
    } = manager.state().await
    else {
        unreachable!()
    };
    assert!(message.contains("404"), "{message}");
}

#[tokio::test]
async fn ensure_payload_ready_stream_error_sets_payload_error() {
    use tokio::io::AsyncWriteExt;

    // Content-Length より少ないバイト数で接続を閉じ、受信エラーを起こす。
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buf).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n0123456789")
            .await
            .unwrap();
        socket.shutdown().await.unwrap();
    });
    let (manager, _tmp) = test_manager_with_gate(InferenceGate::default());
    seed_available_with_portable_url(&manager, format!("http://{addr}/portable.tar.gz")).await;

    let result = manager.ensure_payload_ready().await;
    server.await.unwrap();

    assert_payload_error(&manager, result, "Failed to download update payload").await;
}

#[tokio::test]
async fn ensure_payload_ready_rename_failure_sets_payload_error() {
    let mock_server = wiremock::MockServer::start().await;
    let url = mock_portable_asset(
        &mock_server,
        wiremock::ResponseTemplate::new(200).set_body_bytes(vec![0u8; 100]),
    )
    .await;
    let (manager, _tmp) = test_manager_with_gate(InferenceGate::default());
    seed_available_with_portable_url(&manager, url).await;

    // rename 先に空でないディレクトリを置き、一時ファイルの rename を失敗させる。
    let blocker = manager
        .inner
        .updates_dir
        .join("4.5.1")
        .join("portable.tar.gz");
    fs::create_dir_all(&blocker).unwrap();
    fs::write(blocker.join("occupied"), b"x").unwrap();

    let result = manager.ensure_payload_ready().await;

    assert_payload_error(&manager, result, "Failed to download update payload").await;
}

#[tokio::test]
async fn ensure_payload_ready_extract_failure_sets_payload_error() {
    let mock_server = wiremock::MockServer::start().await;
    // tar.gz ではないゼロ列を返し、展開を失敗させる。
    let url = mock_portable_asset(
        &mock_server,
        wiremock::ResponseTemplate::new(200).set_body_bytes(vec![0u8; 100]),
    )
    .await;
    let (manager, _tmp) = test_manager_with_gate(InferenceGate::default());
    seed_available_with_portable_url(&manager, url).await;

    let result = manager.ensure_payload_ready().await;

    assert_payload_error(&manager, result, "Failed to extract update payload").await;
}

#[tokio::test]
async fn ensure_payload_ready_missing_binary_sets_payload_error() {
    let mock_server = wiremock::MockServer::start().await;
    let archive = tar_gz_with_files(&[("llmlb-test/README.md", b"no binary here")]);
    let url = mock_portable_asset(
        &mock_server,
        wiremock::ResponseTemplate::new(200).set_body_bytes(archive),
    )
    .await;
    let (manager, _tmp) = test_manager_with_gate(InferenceGate::default());
    seed_available_with_portable_url(&manager, url).await;

    let result = manager.ensure_payload_ready().await;

    assert_payload_error(&manager, result, "did not contain").await;
}

#[tokio::test]
async fn ensure_payload_ready_success_sets_payload_ready() {
    let mock_server = wiremock::MockServer::start().await;
    let binary_name = Platform::detect().unwrap().binary_name();
    let entry = format!("llmlb-test/{binary_name}");
    let archive = tar_gz_with_files(&[(entry.as_str(), b"#!/bin/sh\n")]);
    let url = mock_portable_asset(
        &mock_server,
        wiremock::ResponseTemplate::new(200).set_body_bytes(archive),
    )
    .await;
    let (manager, _tmp) = test_manager_with_gate(InferenceGate::default());
    seed_available_with_portable_url(&manager, url).await;

    let kind = manager
        .ensure_payload_ready()
        .await
        .expect("payload preparation should succeed");

    let PayloadKind::Portable { binary_path } = &kind else {
        panic!("expected portable payload, got {kind:?}");
    };
    // 区切り文字は OS で異なるため、文字列ではなくパス要素で比較する。
    let expected_suffix = std::path::Path::new("llmlb-test").join(&binary_name);
    assert!(
        std::path::Path::new(binary_path).ends_with(&expected_suffix),
        "unexpected binary path {binary_path}"
    );
    match manager.state().await {
        UpdateState::Available {
            payload: PayloadState::Ready { kind: ready },
            ..
        } => assert_eq!(ready, kind),
        other => panic!("expected payload Ready, got {other:?}"),
    }
}

// =======================================================================
// asset_name_from_url
// =======================================================================
#[test]
fn asset_name_from_url_extracts_filename() {
    assert_eq!(
        asset_name_from_url("https://example.com/downloads/llmlb-linux-x86_64.tar.gz"),
        Some("llmlb-linux-x86_64.tar.gz".to_string())
    );
}

#[test]
fn asset_name_from_url_single_segment() {
    assert_eq!(
        asset_name_from_url("llmlb.tar.gz"),
        Some("llmlb.tar.gz".to_string())
    );
}

#[test]
fn asset_name_from_url_empty() {
    assert_eq!(asset_name_from_url(""), Some("".to_string()));
}

// =======================================================================
// find_extracted_binary
// =======================================================================
#[test]
fn find_extracted_binary_at_root() {
    let dir = tempfile::tempdir().unwrap();
    let binary_path = dir.path().join("llmlb");
    fs::write(&binary_path, b"binary content").unwrap();

    let result = find_extracted_binary(dir.path(), "llmlb").unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), binary_path);
}

#[test]
fn find_extracted_binary_in_subdir() {
    let dir = tempfile::tempdir().unwrap();
    let sub_dir = dir.path().join("llmlb-linux-x86_64");
    fs::create_dir_all(&sub_dir).unwrap();
    let binary_path = sub_dir.join("llmlb");
    fs::write(&binary_path, b"binary content").unwrap();

    let result = find_extracted_binary(dir.path(), "llmlb").unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap(), binary_path);
}

#[test]
fn find_extracted_binary_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let result = find_extracted_binary(dir.path(), "llmlb").unwrap();
    assert!(result.is_none());
}

#[test]
fn find_extracted_binary_deep_nested() {
    let dir = tempfile::tempdir().unwrap();
    let deep_dir = dir.path().join("a").join("b").join("c");
    fs::create_dir_all(&deep_dir).unwrap();
    let binary_path = deep_dir.join("llmlb");
    fs::write(&binary_path, b"binary content").unwrap();

    let result = find_extracted_binary(dir.path(), "llmlb").unwrap();
    assert!(result.is_some());
}

// =======================================================================
// extract_archive: unsupported format
// =======================================================================
#[test]
fn extract_archive_unsupported_format_fails() {
    let dir = tempfile::tempdir().unwrap();
    let archive_path = dir.path().join("archive.7z");
    fs::write(&archive_path, b"some content").unwrap();
    let dest = dir.path().join("extract");
    fs::create_dir_all(&dest).unwrap();

    let result = extract_archive(&archive_path, &dest);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("unsupported archive format"));
}
