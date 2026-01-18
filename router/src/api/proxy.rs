//! LLM runtimeプロキシ APIハンドラー
//!
//! # 移行中
//!
//! このモジュールは現在、Node型からEndpoint型への移行期間中です。

#![allow(deprecated)] // Using deprecated Node type during EndpointRegistry migration

use crate::{config::QueueConfig, AppState};
use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, StatusCode},
    response::Response,
};
use futures::TryStreamExt;
use llm_router_common::{error::RouterError, protocol::RequestResponseRecord};
use std::{io, sync::Arc, time::Instant};

use crate::balancer::WaitResult;

pub(crate) async fn select_available_node(
    state: &AppState,
) -> Result<llm_router_common::types::Node, RouterError> {
    let mode = std::env::var("LOAD_BALANCER_MODE").unwrap_or_else(|_| "auto".to_string());

    match mode.as_str() {
        "metrics" => {
            // メトリクスベース選択（T014-T015で実装）
            state.load_manager.select_endpoint_by_metrics().await
        }
        _ => {
            // デフォルト: 既存の高度なロードバランシング
            let node = state.load_manager.select_endpoint().await?;
            if node.initializing {
                return Err(RouterError::ServiceUnavailable(
                    "All nodes are warming up models".into(),
                ));
            }
            Ok(node)
        }
    }
}

pub(crate) enum QueueSelection {
    Ready {
        node: Box<llm_router_common::types::Node>,
        queued_wait_ms: Option<u128>,
    },
    CapacityExceeded,
    Timeout {
        waited_ms: u128,
    },
}

pub(crate) async fn select_available_node_with_queue_for_model(
    state: &AppState,
    queue_config: QueueConfig,
    model_id: &str,
) -> Result<QueueSelection, RouterError> {
    match state
        .load_manager
        .select_idle_node_for_model(model_id)
        .await?
    {
        Some(node) => Ok(QueueSelection::Ready {
            node: Box::new(node),
            queued_wait_ms: None,
        }),
        None => {
            let wait_start = Instant::now();
            match state
                .load_manager
                .wait_for_idle_node_with_timeout_for_model(
                    model_id,
                    queue_config.max_waiters,
                    queue_config.timeout,
                )
                .await
            {
                WaitResult::CapacityExceeded => Ok(QueueSelection::CapacityExceeded),
                WaitResult::Timeout => Ok(QueueSelection::Timeout {
                    waited_ms: wait_start.elapsed().as_millis(),
                }),
                WaitResult::Ready => match state
                    .load_manager
                    .select_idle_node_for_model(model_id)
                    .await?
                {
                    Some(node) => Ok(QueueSelection::Ready {
                        node: Box::new(node),
                        queued_wait_ms: Some(wait_start.elapsed().as_millis()),
                    }),
                    None => Err(RouterError::NoNodesAvailable),
                },
            }
        }
    }
}

pub(crate) fn forward_streaming_response(
    response: reqwest::Response,
) -> Result<Response, RouterError> {
    let status = response.status();
    let headers = response.headers().clone();
    let stream = response.bytes_stream().map_err(io::Error::other);
    let body = Body::from_stream(stream);
    let mut axum_response = Response::new(body);
    *axum_response.status_mut() = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK);
    {
        let response_headers = axum_response.headers_mut();
        for (name, value) in headers.iter() {
            if let (Ok(header_name), Ok(header_value)) = (
                HeaderName::from_bytes(name.as_str().as_bytes()),
                HeaderValue::from_bytes(value.as_bytes()),
            ) {
                response_headers.insert(header_name, header_value);
            }
        }
    }
    use axum::http::header;
    if !axum_response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap_or("").starts_with("text/event-stream"))
        .unwrap_or(false)
    {
        axum_response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    Ok(axum_response)
}

/// リクエスト/レスポンスレコードを保存（Fire-and-forget）
pub(crate) fn save_request_record(
    storage: Arc<crate::db::request_history::RequestHistoryStorage>,
    record: RequestResponseRecord,
) {
    tokio::spawn(async move {
        if let Err(e) = storage.save_record(&record).await {
            tracing::error!("Failed to save request record: {}", e);
        }
    });
}

/// エンドポイント選択結果
pub(crate) enum EndpointSelection {
    /// エンドポイントが見つかった（Boxでヒープ割り当て、enum sizeの最適化）
    Found(Box<crate::types::endpoint::Endpoint>),
    /// モデルをサポートするエンドポイントがない
    NotFound,
}

/// モデルIDからエンドポイントを選択（レイテンシ順）
///
/// EndpointRegistryからモデルをサポートするオンラインエンドポイントを検索し、
/// 最もレイテンシが低いものを返す。
pub(crate) async fn select_endpoint_for_model(
    state: &AppState,
    model_id: &str,
) -> Result<EndpointSelection, RouterError> {
    let endpoints = state
        .endpoint_registry
        .find_by_model_sorted_by_latency(model_id)
        .await;

    match endpoints.into_iter().next() {
        Some(endpoint) => Ok(EndpointSelection::Found(Box::new(endpoint))),
        None => Ok(EndpointSelection::NotFound),
    }
}

/// エンドポイントにリクエストを転送
///
/// OpenAI互換APIエンドポイントにリクエストを転送し、レスポンスを返す
pub(crate) async fn forward_to_endpoint(
    client: &reqwest::Client,
    endpoint: &crate::types::endpoint::Endpoint,
    path: &str,
    body: Vec<u8>,
    stream: bool,
) -> Result<reqwest::Response, RouterError> {
    let url = format!("{}{}", endpoint.base_url.trim_end_matches('/'), path);

    let mut request_builder = client
        .post(&url)
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(
            endpoint.inference_timeout_secs as u64,
        ))
        .body(body);

    // APIキーがあれば追加
    if let Some(api_key) = &endpoint.api_key {
        request_builder = request_builder.bearer_auth(api_key);
    }

    let response = request_builder.send().await.map_err(|e| {
        tracing::error!(
            "Failed to forward request to endpoint {}: {}",
            endpoint.name,
            e
        );
        RouterError::Http(format!("Endpoint request failed: {}", e))
    })?;

    // エラーステータスをチェック
    let status = response.status();
    if !status.is_success() && !stream {
        // 非ストリーミングの場合はエラー内容を取得してログ
        let error_body = response.text().await.unwrap_or_default();
        tracing::warn!(
            "Endpoint {} returned error {}: {}",
            endpoint.name,
            status,
            error_body
        );
        return Err(RouterError::Http(format!(
            "Endpoint returned {}: {}",
            status, error_body
        )));
    }

    Ok(response)
}

// NOTE: テストはNodeRegistry廃止に伴い削除されました。
// 新しいテストはEndpointRegistryベースで tests/integration/ に追加してください。
