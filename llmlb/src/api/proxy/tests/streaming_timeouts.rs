//! Issue #978: ストリーミング中継のタイムアウト方針テスト
//!
//! アイドル（無応答）タイムアウトと、ストリーミングに総量タイムアウトを
//! 適用しない方針（オーナー決定 2026-11-26）の検証。proxy.rs の行数上限
//! （FR-009）のため tests 配下へ分離した。

use super::*;

// --- Issue #978: ストリーミング中継のアイドル（無応答）タイムアウト ---

/// chunked encoding の SSE を返すテスト用上流サーバーを起動する。
///
/// `steps` は（送信前遅延ミリ秒, チャンク本文）の列。送信後に `hang` が
/// true なら終端チャンクを送らずに接続を保持し続け、false なら終端
/// チャンクを送って接続を閉じる。
async fn spawn_chunked_sse_server(
    steps: Vec<(u64, &'static str)>,
    hang: bool,
) -> std::net::SocketAddr {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut read_buf = [0u8; 4096];
            let _ = socket.read(&mut read_buf).await;
            let headers = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n";
            if socket.write_all(headers.as_bytes()).await.is_err() {
                return;
            }
            for (delay_ms, chunk) in steps {
                if delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                let framed = format!("{:x}\r\n{}\r\n", chunk.len(), chunk);
                if socket.write_all(framed.as_bytes()).await.is_err() {
                    return;
                }
                let _ = socket.flush().await;
            }
            if hang {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            } else {
                let _ = socket.write_all(b"0\r\n\r\n").await;
                let _ = socket.shutdown().await;
            }
        }
    });
    addr
}

/// ストリーミング検証用に、レジストリ・LoadManager・イベントバス・リースを用意する。
async fn setup_streaming_lease(
    name: &str,
) -> (
    crate::registry::endpoints::EndpointRegistry,
    crate::balancer::LoadManager,
    crate::events::SharedEventBus,
    uuid::Uuid,
    crate::balancer::RequestLease,
) {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("create test db");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    let registry = crate::registry::endpoints::EndpointRegistry::new(pool)
        .await
        .expect("create registry");
    let endpoint = Endpoint::new(
        name.to_string(),
        "http://localhost:1".to_string(),
        crate::types::endpoint::EndpointType::OpenaiCompatible,
    );
    let endpoint_id = endpoint.id;
    registry.add(endpoint).await.expect("add endpoint");
    let load_manager = crate::balancer::LoadManager::new(Arc::new(registry.clone()));
    let event_bus = crate::events::create_shared_event_bus();
    let lease = load_manager
        .begin_request(endpoint_id)
        .await
        .expect("begin request");
    (registry, load_manager, event_bus, endpoint_id, lease)
}

#[tokio::test]
async fn streaming_forwarder_aborts_when_upstream_goes_idle() {
    let addr = spawn_chunked_sse_server(
        vec![(
            0,
            "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        )],
        true,
    )
    .await;
    let (registry, load_manager, event_bus, endpoint_id, lease) =
        setup_streaming_lease("idle-stream-ep").await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/v1/chat/completions"))
        .body("{}")
        .send()
        .await
        .expect("upstream response");

    let axum_response = forward_streaming_response_with_tps_tracking(
        response,
        endpoint_id,
        "m".to_string(),
        None,
        crate::types::endpoint::EndpointType::OpenaiCompatible,
        Instant::now(),
        registry,
        load_manager.clone(),
        event_bus,
        Some(lease),
        std::time::Duration::from_millis(150),
    )
    .expect("forward streaming");

    // 上流がチャンク送信後に無応答になっても、アイドルタイムアウトで
    // ストリームが打ち切られ、総量タイムアウトまでぶら下がらない。
    let started = Instant::now();
    let consumed = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        axum::body::to_bytes(axum_response.into_body(), usize::MAX),
    )
    .await
    .expect("stream must terminate via idle timeout, not hang");
    assert!(
        consumed.is_err(),
        "アイドル打ち切りはストリームエラーとして伝播するべき"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "総量タイムアウトを待たずに打ち切られるべき"
    );

    // 失敗としてリースが完了し、実行中カウントが残留しない。
    assert_eq!(
        load_manager
            .snapshot(endpoint_id)
            .await
            .unwrap()
            .active_requests,
        0,
        "lease must be completed after idle abort"
    );
}

#[tokio::test]
async fn streaming_forwarder_tolerates_gaps_below_idle_timeout() {
    let addr = spawn_chunked_sse_server(
        vec![
            (
                0,
                "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n",
            ),
            (
                80,
                "data: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\n",
            ),
            (80, "data: [DONE]\n\n"),
        ],
        false,
    )
    .await;
    let (registry, load_manager, event_bus, endpoint_id, lease) =
        setup_streaming_lease("progressing-stream-ep").await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/v1/chat/completions"))
        .body("{}")
        .send()
        .await
        .expect("upstream response");

    let axum_response = forward_streaming_response_with_tps_tracking(
        response,
        endpoint_id,
        "m".to_string(),
        None,
        crate::types::endpoint::EndpointType::OpenaiCompatible,
        Instant::now(),
        registry,
        load_manager.clone(),
        event_bus,
        Some(lease),
        std::time::Duration::from_secs(2),
    )
    .expect("forward streaming");

    // チャンク間隔がアイドルタイムアウト未満なら、間隔が空いても切断されない。
    let bytes = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        axum::body::to_bytes(axum_response.into_body(), usize::MAX),
    )
    .await
    .expect("stream must complete")
    .expect("consume body");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("[DONE]"),
        "ストリーム全体が中継されるべき: {text}"
    );
    assert_eq!(
        load_manager
            .snapshot(endpoint_id)
            .await
            .unwrap()
            .active_requests,
        0,
        "lease must be completed after the stream completes"
    );
}

// --- Issue #978 フォローアップ: ストリーミングは総量タイムアウトで切らない ---

/// 応答全体（ヘッダー含む）を遅延させてから返すテスト用上流サーバーを起動する。
async fn spawn_delayed_response_server(delay_ms: u64, body: &'static str) -> std::net::SocketAddr {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut read_buf = [0u8; 4096];
            let _ = socket.read(&mut read_buf).await;
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    addr
}

/// forward_to_endpoint のタイムアウト検証用エンドポイント（総量は秒単位で指定）。
fn forward_test_endpoint(addr: std::net::SocketAddr, inference_timeout_secs: u32) -> Endpoint {
    let mut ep = Endpoint::new(
        "fwd-ep".to_string(),
        format!("http://{addr}"),
        crate::types::endpoint::EndpointType::OpenaiCompatible,
    );
    ep.inference_timeout_secs = inference_timeout_secs;
    ep
}

#[tokio::test]
async fn forward_to_endpoint_streaming_is_not_cut_by_total_timeout() {
    // 総量タイムアウト（1秒）より長く続くストリームでも、チャンク間隔が
    // アイドルタイムアウト未満なら最後まで中継される（オーナー決定 2026-11-26）。
    let addr = spawn_chunked_sse_server(
        vec![
            (
                0,
                "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n",
            ),
            (
                800,
                "data: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\n",
            ),
            (800, "data: [DONE]\n\n"),
        ],
        false,
    )
    .await;
    let (registry, load_manager, event_bus, endpoint_id, lease) =
        setup_streaming_lease("fwd-stream-ep").await;
    let endpoint = forward_test_endpoint(addr, 1);

    let started = Instant::now();
    let response = forward_to_endpoint(
        &reqwest::Client::new(),
        &endpoint,
        "/v1/chat/completions",
        b"{}".to_vec(),
        true,
    )
    .await
    .expect("forward streaming request");

    let axum_response = forward_streaming_response_with_tps_tracking(
        response,
        endpoint_id,
        "m".to_string(),
        None,
        crate::types::endpoint::EndpointType::OpenaiCompatible,
        Instant::now(),
        registry,
        load_manager.clone(),
        event_bus,
        Some(lease),
        std::time::Duration::from_secs(5),
    )
    .expect("forward streaming");

    let bytes = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        axum::body::to_bytes(axum_response.into_body(), usize::MAX),
    )
    .await
    .expect("stream must complete")
    .expect("ストリームが総量タイムアウトで切断されないべき");
    assert!(
        started.elapsed() > std::time::Duration::from_secs(1),
        "総量タイムアウト（1秒）を越えて継続したはず"
    );
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("[DONE]"),
        "ストリーム全体が中継されるべき: {text}"
    );
    assert_eq!(
        load_manager
            .snapshot(endpoint_id)
            .await
            .unwrap()
            .active_requests,
        0,
        "lease must be completed after the stream completes"
    );
}

#[tokio::test]
async fn forward_to_endpoint_streaming_waits_beyond_total_timeout_for_headers() {
    // ヘッダー到着が総量タイムアウト（1秒）より遅くても、ストリーミングでは
    // 総量で打ち切らずに待てる（無応答の検出は中継側のアイドルが担う）。
    let addr = spawn_delayed_response_server(2000, "{}").await;
    let endpoint = forward_test_endpoint(addr, 1);

    let started = Instant::now();
    let response = forward_to_endpoint(
        &reqwest::Client::new(),
        &endpoint,
        "/v1/chat/completions",
        b"{}".to_vec(),
        true,
    )
    .await
    .expect("ストリーミングではヘッダー待ちに総量タイムアウトが効かないべき");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(1500),
        "ヘッダー到着（2秒）まで待ったはず"
    );
}

#[tokio::test]
async fn forward_to_endpoint_non_streaming_times_out_on_slow_headers() {
    // 非ストリーミングは従来どおり総量タイムアウトが絶対上限として効く。
    let addr = spawn_delayed_response_server(2000, "{}").await;
    let endpoint = forward_test_endpoint(addr, 1);

    let result = forward_to_endpoint(
        &reqwest::Client::new(),
        &endpoint,
        "/v1/chat/completions",
        b"{}".to_vec(),
        false,
    )
    .await;
    match result {
        Err(LbError::Timeout(_)) => {}
        Err(other) => panic!("expected timeout error, got: {other}"),
        Ok(_) => panic!("expected timeout error, got a response"),
    }
}

#[tokio::test]
async fn forward_to_endpoint_non_streaming_body_stall_times_out() {
    // ヘッダー後に無応答になる上流でも、非ストリーミングは総量で打ち切られる。
    let addr = spawn_chunked_sse_server(
        vec![(
            0,
            "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n",
        )],
        true,
    )
    .await;
    let endpoint = forward_test_endpoint(addr, 1);

    let response = forward_to_endpoint(
        &reqwest::Client::new(),
        &endpoint,
        "/v1/chat/completions",
        b"{}".to_vec(),
        false,
    )
    .await
    .expect("headers arrive immediately");

    let started = Instant::now();
    let body_result = tokio::time::timeout(std::time::Duration::from_secs(10), response.bytes())
        .await
        .expect("総量タイムアウトで打ち切られるべき（ハングしない）");
    assert!(
        body_result.is_err(),
        "ボディ読み取りはタイムアウトエラーになるべき"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "総量タイムアウト（1秒）付近で打ち切られるべき"
    );
}
