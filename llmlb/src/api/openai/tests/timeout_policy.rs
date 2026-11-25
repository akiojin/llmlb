//! Issue #978 フォローアップ（オーナー決定 2026-11-26）:
//! ストリーミングは総量タイムアウトで切らず、アイドルタイムアウトのみで管理する。
//! 非ストリーミングは総量タイムアウトが絶対上限として引き続き効く。

use super::*;

/// chunked encoding の SSE を返すテスト用上流サーバーを起動する。
///
/// `steps` は（送信前遅延ミリ秒, チャンク本文）の列。送信後は終端チャンクを
/// 送って接続を閉じる。
async fn spawn_chunked_sse_server(steps: Vec<(u64, &'static str)>) -> std::net::SocketAddr {
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
            let _ = socket.write_all(b"0\r\n\r\n").await;
            let _ = socket.shutdown().await;
        }
    });
    addr
}

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

#[tokio::test]
#[serial]
async fn streaming_chat_completion_is_not_cut_by_total_timeout() {
    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    // 総量タイムアウトは1秒。チャンク間隔（800ms）はアイドル未満だが、
    // ストリーム全体は約1.6秒続く。
    let addr = spawn_chunked_sse_server(vec![
        (
            0,
            "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n",
        ),
        (
            800,
            "data: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\n",
        ),
        (800, "data: [DONE]\n\n"),
    ])
    .await;
    add_online_chat_endpoint(
        &state,
        "timeout-policy-stream",
        format!("http://{addr}"),
        "timeout-policy-model",
        1,
    )
    .await;

    let payload = json!({
        "model": "timeout-policy-model",
        "messages": [{"role": "user", "content": "hello"}],
        "stream": true
    });
    let started = std::time::Instant::now();
    let response = proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        "timeout-policy-model".to_string(),
        true,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("streaming request should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = tokio::time::timeout(
        Duration::from_secs(10),
        to_bytes(response.into_body(), 1_000_000),
    )
    .await
    .expect("stream must complete")
    .expect("ストリームが総量タイムアウトで切断されないべき");
    assert!(
        started.elapsed() > Duration::from_secs(1),
        "総量タイムアウト（1秒）を越えて継続したはず"
    );
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("[DONE]"),
        "ストリーム全体が中継されるべき: {text}"
    );
}

#[tokio::test]
#[serial]
async fn non_streaming_chat_completion_is_bounded_by_total_timeout() {
    let _guard = TEST_LOCK.lock().await;
    let (state, _dir) = create_state_with_tempdir().await;

    // 総量タイムアウトは1秒。上流は2秒遅れて応答するため、非ストリーミングは
    // 従来どおり総量で打ち切られ、504 が返る。
    let addr = spawn_delayed_response_server(2000, "{\"id\":\"x\",\"choices\":[]}").await;
    add_online_chat_endpoint(
        &state,
        "timeout-policy-nonstream",
        format!("http://{addr}"),
        "timeout-policy-model-ns",
        1,
    )
    .await;

    let payload = json!({
        "model": "timeout-policy-model-ns",
        "messages": [{"role": "user", "content": "hello"}]
    });
    let response = proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        "timeout-policy-model-ns".to_string(),
        false,
        RequestType::Chat,
        None,
        None,
    )
    .await
    .expect("request handling should return a response");
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
}
