//! Contract Test: ノード登録のAPIキー必須化 (POST /api/nodes)

use crate::support::router::spawn_test_router;
use reqwest::{Client, StatusCode as ReqStatusCode};
use serde_json::json;

#[tokio::test]
async fn test_node_registration_requires_api_key() {
    let router = spawn_test_router().await;

    let resp = Client::new()
        .post(format!("http://{}/api/nodes", router.addr()))
        .json(&json!({
            "machine_name": "stub-node",
            "ip_address": "127.0.0.1",
            "runtime_version": "0.0.0-test",
            "runtime_port": 11434,
            "gpu_available": true,
            "gpu_devices": [
                {"model": "Test GPU", "count": 1, "memory": 16_000_000_000u64}
            ]
        }))
        .send()
        .await
        .expect("request should succeed");

    assert_eq!(
        resp.status(),
        ReqStatusCode::UNAUTHORIZED,
        "expected 401 when missing API key"
    );
}
