use super::{
    extract_client_ip_from_headers, parse_client_ip_from_forwarded_value, parse_cloud_model,
    proxy_openai_cloud_post,
};
use crate::common::protocol::{RecordStatus, RequestType};
use crate::{
    db::test_utils::{TestAppStateBuilder, TEST_LOCK},
    AppState,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::{body::to_bytes, Json};
use serde_json::json;
use serial_test::serial;
use std::net::{IpAddr, SocketAddr};
use tempfile::tempdir;
use tokio::time::{sleep, Duration};
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
// 子モジュールの関数内 `use super::X` を、分割前と同じ名前で解決させる
use super::*;

mod client_ip;
mod cloud_prefix;
mod list_models;
mod payload;
mod routing;
mod streaming_tps;

// Direct forwarder tests start after authentication, but still use the real
// receipt/admission/Body observation fixture rather than omitting accounting.
#[allow(clippy::too_many_arguments)]
async fn proxy_openai_post(
    state: &AppState,
    payload: serde_json::Value,
    target_path: &str,
    model: String,
    stream: bool,
    request_type: RequestType,
    client_ip: Option<IpAddr>,
    api_key_id: Option<uuid::Uuid>,
) -> Result<axum::response::Response, crate::api::error::OpenAIError> {
    crate::api::usage_capture::with_test_capture(
        state,
        client_ip,
        api_key_id,
        super::proxy_openai_post(
            state,
            payload,
            target_path,
            model,
            stream,
            request_type,
            client_ip,
            api_key_id,
        ),
    )
    .await
}

async fn chat_completions(
    address: axum::extract::ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::extract::State(state): axum::extract::State<AppState>,
    auth: Option<axum::Extension<crate::auth::middleware::ApiKeyAuthContext>>,
    payload: Json<serde_json::Value>,
) -> Result<axum::response::Response, crate::api::error::OpenAIError> {
    let (client_ip, api_key_id) = super::extract_client_info(&address.0, &headers, &auth);
    crate::api::usage_capture::with_test_capture(
        &state,
        client_ip,
        api_key_id,
        super::chat_completions(
            address,
            headers,
            axum::extract::State(state.clone()),
            auth,
            payload,
        ),
    )
    .await
}

async fn create_local_state() -> AppState {
    TestAppStateBuilder::new().await.build().await
}

async fn create_state_with_tempdir() -> (AppState, tempfile::TempDir) {
    let dir = tempdir().expect("temp dir");
    std::env::set_var("LLMLB_DATA_DIR", dir.path());
    let state = create_local_state().await;
    (state, dir)
}

/// fire-and-forget の履歴保存（save_request_record）が反映されるまで待つ。
/// 固定 sleep は高負荷時に保存完了より先に読んでしまうため、条件成立までポーリングする。
async fn wait_for_history_records(
    state: &AppState,
    ready: impl Fn(&[crate::common::protocol::RequestResponseRecord]) -> bool,
) -> Vec<crate::common::protocol::RequestResponseRecord> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let records = state
            .balancer
            .request_history
            .load_records()
            .await
            .expect("records");
        if ready(&records) || tokio::time::Instant::now() >= deadline {
            return records;
        }
        sleep(Duration::from_millis(20)).await;
    }
}

async fn add_online_chat_endpoint(
    state: &AppState,
    endpoint_name: &str,
    base_url: String,
    model_id: &str,
    inference_timeout_secs: u32,
) -> uuid::Uuid {
    add_online_chat_endpoint_with_type(
        state,
        endpoint_name,
        base_url,
        model_id,
        inference_timeout_secs,
        crate::types::endpoint::EndpointType::OpenaiCompatible,
    )
    .await
}

async fn add_online_chat_endpoint_with_type(
    state: &AppState,
    endpoint_name: &str,
    base_url: String,
    model_id: &str,
    inference_timeout_secs: u32,
    endpoint_type: crate::types::endpoint::EndpointType,
) -> uuid::Uuid {
    use crate::types::endpoint::SupportedAPI;

    add_online_chat_endpoint_with_supported_apis(
        state,
        endpoint_name,
        base_url,
        model_id,
        inference_timeout_secs,
        endpoint_type,
        vec![SupportedAPI::ChatCompletions],
    )
    .await
}

async fn add_online_chat_endpoint_with_supported_apis(
    state: &AppState,
    endpoint_name: &str,
    base_url: String,
    model_id: &str,
    inference_timeout_secs: u32,
    endpoint_type: crate::types::endpoint::EndpointType,
    supported_apis: Vec<crate::types::endpoint::SupportedAPI>,
) -> uuid::Uuid {
    use crate::types::endpoint::{Endpoint, EndpointModel, EndpointStatus};

    let mut endpoint = Endpoint::new(endpoint_name.to_string(), base_url, endpoint_type);
    endpoint.status = EndpointStatus::Online;
    endpoint.inference_timeout_secs = inference_timeout_secs;
    let endpoint_id = endpoint.id;
    state
        .balancer
        .endpoint_registry
        .add(endpoint)
        .await
        .expect("add endpoint");
    state
        .balancer
        .endpoint_registry
        .add_model(&EndpointModel {
            endpoint_id,
            model_id: model_id.to_string(),
            capabilities: None,
            max_tokens: None,
            last_checked: None,
            supported_apis,
            canonical_name: None,
        })
        .await
        .expect("add endpoint model");
    endpoint_id
}
