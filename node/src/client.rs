//! Coordinator通信クライアント
//!
//! ノード登録とハートビート送信

use ollama_router_common::{
    error::{NodeError, NodeResult},
    protocol::{HealthCheckRequest, RegisterRequest, RegisterResponse},
};
use tracing::info;
use uuid::Uuid;

/// Coordinatorクライアント
pub struct CoordinatorClient {
    router_url: String,
    node_id: Option<Uuid>,
    http_client: reqwest::Client,
}

impl CoordinatorClient {
    /// 新しいCoordinatorクライアントを作成
    pub fn new(router_url: String) -> Self {
        Self {
            router_url,
            node_id: None,
            http_client: reqwest::Client::new(),
        }
    }

    /// ノードを登録
    pub async fn register(&mut self, req: RegisterRequest) -> NodeResult<RegisterResponse> {
        let url = format!("{}/api/nodes", self.router_url);

        let response = self
            .http_client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| NodeError::CoordinatorConnection(format!("Failed to register: {}", e)))?;

        if !response.status().is_success() {
            return Err(NodeError::Registration(format!(
                "Registration failed with status: {}",
                response.status()
            )));
        }

        let register_response = response
            .json::<RegisterResponse>()
            .await
            .map_err(|e| NodeError::Registration(format!("Failed to parse response: {}", e)))?;

        // ノードIDを保存
        self.node_id = Some(register_response.node_id);

        info!(
            "Registered with Coordinator: node_id = {}",
            register_response.node_id
        );

        Ok(register_response)
    }

    /// ヘルスチェックを送信
    pub async fn send_heartbeat(&self, req: HealthCheckRequest) -> NodeResult<()> {
        let url = format!("{}/api/health", self.router_url);

        let response = self
            .http_client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| NodeError::Heartbeat(format!("Failed to send heartbeat: {}", e)))?;

        if !response.status().is_success() {
            return Err(NodeError::Heartbeat(format!(
                "Heartbeat failed with status: {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// 保存されたノードIDを取得
    pub fn get_node_id(&self) -> Option<Uuid> {
        self.node_id
    }

    /// 利用中のルーターURLを取得
    pub fn router_url(&self) -> &str {
        &self.router_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_client_creation() {
        let client = CoordinatorClient::new("http://localhost:8080".to_string());
        assert_eq!(client.router_url, "http://localhost:8080");
        assert!(client.node_id.is_none());
    }

    #[test]
    fn test_get_node_id_none() {
        let client = CoordinatorClient::new("http://localhost:8080".to_string());
        assert!(client.get_node_id().is_none());
    }
}
