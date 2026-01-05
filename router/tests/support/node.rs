//! モックノードサーバーヘルパー
//!
//! テストで使用するモックHTTPサーバーを提供し、
//! ノード登録時のヘルスチェックに応答します。

use serde_json::json;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

/// ノード登録テスト用のモックサーバー
#[allow(dead_code)] // Prepared for future integration tests
pub struct MockNodeServer {
    pub server: MockServer,
    pub port: u16,
    pub runtime_port: u16,
}

#[allow(dead_code)] // Prepared for future integration tests
impl MockNodeServer {
    /// モックノードサーバーを起動
    ///
    /// ルーターは runtime_port + 1 をAPIポートとして使用するため、
    /// runtime_port = mock_port - 1 を返します。
    pub async fn start() -> Self {
        let server = MockServer::start().await;

        // /v1/models エンドポイントをモック
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "object": "list",
                "data": [{"id": "mock-model"}]
            })))
            .mount(&server)
            .await;

        let port = server.address().port();
        let runtime_port = port - 1;

        Self {
            server,
            port,
            runtime_port,
        }
    }

    /// IPアドレスを返す（常に127.0.0.1）
    pub fn ip_address(&self) -> &'static str {
        "127.0.0.1"
    }
}
