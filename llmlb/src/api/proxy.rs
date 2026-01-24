//! LLM runtimeプロキシ APIハンドラー
//!
//! # SPEC-f8e3a1b7: Endpoint型への移行完了
//!
//! このモジュールはEndpoint型を使用しています。

use crate::common::{error::LbError, protocol::RequestResponseRecord};
use crate::{config::QueueConfig, types::endpoint::Endpoint, AppState};
use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, StatusCode},
    response::Response,
};
use futures::TryStreamExt;
use std::{io, sync::Arc, time::Instant};

use crate::balancer::WaitResult;

/// ラウンドロビンでエンドポイントを選択
///
/// llmlbはゲートウェイとしてエンドポイントをブラックボックスとして扱うため、
/// 負荷分散は単純なラウンドロビン方式を採用しています。
/// 標準のOpenAI互換APIにはメトリクスエンドポイントがないため、
/// エンドポイントの内部状態（VRAM、負荷等）を考慮した選択は行いません。
pub(crate) async fn select_available_endpoint(state: &AppState) -> Result<Endpoint, LbError> {
    state
        .load_manager
        .select_endpoint_round_robin_direct()
        .await
}

/// キュー付きエンドポイント選択の結果
pub(crate) enum QueueSelection {
    /// エンドポイントが見つかった
    Ready {
        endpoint: Box<Endpoint>,
        queued_wait_ms: Option<u128>,
    },
    /// キャパシティ超過
    CapacityExceeded,
    /// タイムアウト
    Timeout { waited_ms: u128 },
}

/// モデル対応のエンドポイントをキュー付きで選択
pub(crate) async fn select_available_endpoint_with_queue_for_model(
    state: &AppState,
    queue_config: QueueConfig,
    model_id: &str,
) -> Result<QueueSelection, LbError> {
    match state
        .load_manager
        .select_idle_endpoint_for_model(model_id)
        .await?
    {
        Some(endpoint) => Ok(QueueSelection::Ready {
            endpoint: Box::new(endpoint),
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
                    .select_idle_endpoint_for_model(model_id)
                    .await?
                {
                    Some(endpoint) => Ok(QueueSelection::Ready {
                        endpoint: Box::new(endpoint),
                        queued_wait_ms: Some(wait_start.elapsed().as_millis()),
                    }),
                    None => Err(LbError::NoNodesAvailable),
                },
            }
        }
    }
}

pub(crate) fn forward_streaming_response(response: reqwest::Response) -> Result<Response, LbError> {
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
    Found(Box<Endpoint>),
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
) -> Result<EndpointSelection, LbError> {
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
    endpoint: &Endpoint,
    path: &str,
    body: Vec<u8>,
    stream: bool,
) -> Result<reqwest::Response, LbError> {
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
        LbError::Http(format!("Endpoint request failed: {}", e))
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
        return Err(LbError::Http(format!(
            "Endpoint returned {}: {}",
            status, error_body
        )));
    }

    Ok(response)
}

// NOTE: テストはNodeRegistry廃止に伴い削除されました。
// 新しいテストはEndpointRegistryベースで tests/integration/ に追加してください。
