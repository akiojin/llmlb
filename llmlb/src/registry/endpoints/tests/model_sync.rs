use super::*;
use crate::common::auth::{Claims, UserRole};
use crate::db::test_utils::TestAppStateBuilder;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Extension,
};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
async fn model_sync_preserves_unrelated_routes_and_refreshes_only_target_models() {
    let _lock = TEST_LOCK.lock().await;
    let state = TestAppStateBuilder::new().await.build().await;
    let registry = &state.balancer.endpoint_registry;
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list", "data": [{"id": "sync-new-model", "object": "model"}]
        })))
        .mount(&server)
        .await;
    let mut ids = Vec::new();
    for (name, model_id) in [
        ("sync-target", "sync-old-model"),
        ("route-owner", "route-owner-model"),
    ] {
        let base_url = if name == "sync-target" {
            server.uri()
        } else {
            format!("{}/unrelated", server.uri())
        };
        let mut endpoint = Endpoint::new(name.to_string(), base_url, EndpointType::Xllm);
        endpoint.status = EndpointStatus::Online;
        let id = endpoint.id;
        registry.add(endpoint).await.unwrap();
        registry
            .add_model(&EndpointModel {
                endpoint_id: id,
                model_id: model_id.to_string(),
                capabilities: None,
                max_tokens: None,
                last_checked: None,
                supported_apis: vec![SupportedAPI::ChatCompletions],
                canonical_name: None,
            })
            .await
            .unwrap();
        ids.push(id);
    }
    // A live route holds a read guard. Syncing another endpoint's models must
    // finish without clearing or exclusively locking the endpoint cache.
    let routes = registry.endpoints.read().await;
    let claims = Claims {
        sub: "admin".to_string(),
        role: UserRole::Admin,
        exp: usize::MAX,
        must_change_password: false,
        password_changed_at: 0,
    };
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        crate::api::endpoints::sync_endpoint_models(
            Extension(claims),
            State(state.clone()),
            Path(ids[0]),
        ),
    )
    .await
    .expect("model sync must not reload the entire endpoint cache")
    .into_response();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert!(routes.contains_key(&ids[1]));
    drop(routes);
    assert_eq!(registry.find_by_model("sync-new-model").await[0].id, ids[0]);
    assert!(registry.find_by_model("sync-old-model").await.is_empty());
    assert_eq!(
        registry.find_by_model("route-owner-model").await[0].id,
        ids[1]
    );
}
