//! Issue #984: クラウド向けストリーミング中継のタイムアウト方針テスト
//!
//! クラウド経路のアイドル（無応答）タイムアウトの検証。proxy.rs の行数
//! 上限（FR-009）のため tests 配下へ分離した。

use super::*;

// --- Issue #984: クラウド向けストリーミング中継のアイドル（無応答）タイムアウト ---

/// chunked encoding の SSE を返すテスト用クラウド上流サーバーを起動する。
///
/// `steps` は（送信前遅延ミリ秒, チャンク本文）の列。送信後に `hang` が
/// true なら終端チャンクを送らずに接続を保持し続け、false なら終端
/// チャンクを送って接続を閉じる。
async fn spawn_cloud_chunked_sse_server(
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

#[tokio::test]
async fn cloud_streaming_relay_aborts_when_upstream_goes_idle() {
    let addr = spawn_cloud_chunked_sse_server(
        vec![(
            0,
            "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
        )],
        true,
    )
    .await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/v1/chat/completions"))
        .body("{}")
        .send()
        .await
        .expect("upstream response");

    let axum_response = forward_streaming_response_with_idle_timeout(
        response,
        std::time::Duration::from_millis(150),
    )
    .expect("forward streaming");

    // 上流がチャンク送信後に無応答になっても、アイドルタイムアウトで
    // ストリームが打ち切られ、無期限に接続が維持されることはない。
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
        "アイドルタイムアウトで速やかに打ち切られるべき"
    );
}

#[tokio::test]
async fn cloud_streaming_relay_aborts_when_first_chunk_never_arrives() {
    let addr = spawn_cloud_chunked_sse_server(vec![], true).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/v1/chat/completions"))
        .body("{}")
        .send()
        .await
        .expect("upstream response");

    let axum_response = forward_streaming_response_with_idle_timeout(
        response,
        std::time::Duration::from_millis(150),
    )
    .expect("forward streaming");

    // ヘッダーだけ届いて初回チャンクが来ない場合も、初回チャンク待ちが
    // アイドルタイムアウトの対象になり打ち切られる。
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
}

#[tokio::test]
async fn cloud_streaming_relay_tolerates_gaps_below_idle_timeout() {
    let addr = spawn_cloud_chunked_sse_server(
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

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/v1/chat/completions"))
        .body("{}")
        .send()
        .await
        .expect("upstream response");

    let axum_response =
        forward_streaming_response_with_idle_timeout(response, std::time::Duration::from_secs(2))
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
}
