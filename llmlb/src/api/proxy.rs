//! LLM runtimeプロキシ APIハンドラー
//!
//! # SPEC-f8e3a1b7: Endpoint型への移行完了
//!
//! このモジュールはEndpoint型を使用しています。

use crate::common::{error::LbError, protocol::RequestResponseRecord};
use crate::token::StreamingTokenAccumulator;
use crate::{config::QueueConfig, types::endpoint::Endpoint, AppState};
use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, StatusCode},
    response::Response,
};
use futures::{Stream, StreamExt, TryStreamExt};
use std::{io, pin::Pin, sync::Arc, time::Instant};

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
#[allow(dead_code)]
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
    _queue_config: QueueConfig,
    model_id: &str,
) -> Result<QueueSelection, LbError> {
    let endpoint = state
        .load_manager
        .select_endpoint_round_robin_ready_for_model(model_id)
        .await?;

    tracing::debug!(
        model = %model_id,
        endpoint_id = %endpoint.id,
        endpoint_name = %endpoint.name,
        "Selected ready endpoint by round-robin"
    );

    Ok(QueueSelection::Ready {
        endpoint: Box::new(endpoint),
        queued_wait_ms: None,
    })
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

fn process_sse_lines(
    buffer: &mut String,
    chunk_text: &str,
    accumulator: &mut StreamingTokenAccumulator,
) {
    buffer.push_str(chunk_text);

    while let Some(newline_idx) = buffer.find('\n') {
        let line = buffer[..newline_idx].trim_end_matches('\r').to_string();
        accumulator.process_chunk(&line);
        buffer.drain(..=newline_idx);
    }
}

/// SSEストリームを透過しながら、完了時にTPS計測用のトークンを集計する。
#[allow(clippy::too_many_arguments)]
pub(crate) fn forward_streaming_response_with_tps_tracking(
    response: reqwest::Response,
    endpoint_id: uuid::Uuid,
    model_id: String,
    endpoint_type: crate::types::endpoint::EndpointType,
    request_started_at: Instant,
    pool: sqlx::SqlitePool,
    load_manager: crate::balancer::LoadManager,
    event_bus: crate::events::SharedEventBus,
) -> Result<Response, LbError> {
    struct TpsTrackingState {
        upstream: Pin<Box<dyn Stream<Item = Result<axum::body::Bytes, reqwest::Error>> + Send>>,
        accumulator: StreamingTokenAccumulator,
        sse_buffer: String,
        endpoint_id: uuid::Uuid,
        model_id: String,
        endpoint_type: crate::types::endpoint::EndpointType,
        request_started_at: Instant,
        pool: sqlx::SqlitePool,
        load_manager: crate::balancer::LoadManager,
        event_bus: crate::events::SharedEventBus,
    }

    let status = response.status();
    let headers = response.headers().clone();

    let state = TpsTrackingState {
        upstream: Box::pin(response.bytes_stream()),
        accumulator: StreamingTokenAccumulator::new(&model_id),
        sse_buffer: String::new(),
        endpoint_id,
        model_id,
        endpoint_type,
        request_started_at,
        pool,
        load_manager,
        event_bus,
    };

    let tracked_stream = futures::stream::try_unfold(state, |mut state| async move {
        match state.upstream.next().await {
            Some(Ok(chunk)) => {
                let chunk_text = String::from_utf8_lossy(chunk.as_ref());
                process_sse_lines(&mut state.sse_buffer, &chunk_text, &mut state.accumulator);
                Ok(Some((chunk, state)))
            }
            Some(Err(err)) => {
                record_endpoint_request_stats(
                    state.pool.clone(),
                    state.endpoint_id,
                    state.model_id.clone(),
                    false,
                    0,
                    0,
                    state.endpoint_type,
                    state.load_manager.clone(),
                    state.event_bus.clone(),
                );
                Err(io::Error::other(err))
            }
            None => {
                if !state.sse_buffer.is_empty() {
                    let pending = std::mem::take(&mut state.sse_buffer);
                    state
                        .accumulator
                        .process_chunk(pending.trim_end_matches('\r'));
                }

                let usage = state.accumulator.finalize();
                let output_tokens = usage.output_tokens.unwrap_or(0) as u64;
                let duration_ms = if output_tokens > 0 {
                    state.request_started_at.elapsed().as_millis().max(1) as u64
                } else {
                    0
                };

                record_endpoint_request_stats(
                    state.pool.clone(),
                    state.endpoint_id,
                    state.model_id.clone(),
                    true,
                    output_tokens,
                    duration_ms,
                    state.endpoint_type,
                    state.load_manager.clone(),
                    state.event_bus.clone(),
                );
                Ok(None)
            }
        }
    });

    let body = Body::from_stream(tracked_stream);
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

/// エンドポイントリクエスト統計を更新（Fire-and-forget）（SPEC-8c32349f）
///
/// endpointsテーブルの累計カウンタとendpoint_daily_statsの日次集計を
/// 非同期で更新する。リクエスト処理のレイテンシに影響を与えない。
/// SPEC-4bb5b55f: TPS計測対象の場合はインメモリEMAも更新する。
#[allow(clippy::too_many_arguments)]
pub(crate) fn record_endpoint_request_stats(
    pool: sqlx::SqlitePool,
    endpoint_id: uuid::Uuid,
    model_id: String,
    success: bool,
    output_tokens: u64,
    duration_ms: u64,
    endpoint_type: crate::types::endpoint::EndpointType,
    load_manager: crate::balancer::LoadManager,
    event_bus: crate::events::SharedEventBus,
) {
    tokio::spawn(async move {
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();

        if let Err(e) =
            crate::db::endpoints::increment_request_counters(&pool, endpoint_id, success).await
        {
            tracing::error!("Failed to increment endpoint request counters: {}", e);
        }

        // TPS計測対象かつ成功かつ有効トークンがある場合のみトークン・時間をDB永続化
        let should_update_tps =
            endpoint_type.is_tps_trackable() && success && output_tokens > 0 && duration_ms > 0;
        let (tokens, duration) = if should_update_tps {
            (output_tokens, duration_ms)
        } else {
            (0, 0)
        };

        if let Err(e) = crate::db::endpoint_daily_stats::upsert_daily_stats(
            &pool,
            endpoint_id,
            &model_id,
            &date,
            success,
            tokens,
            duration,
        )
        .await
        {
            tracing::error!("Failed to upsert daily stats: {}", e);
        }

        // SPEC-4bb5b55f: インメモリTPS EMAを更新 & イベント発行
        if should_update_tps {
            load_manager
                .update_tps(endpoint_id, model_id.clone(), output_tokens, duration_ms)
                .await;

            let tps = output_tokens as f64 / (duration_ms as f64 / 1000.0);
            event_bus.publish(crate::events::DashboardEvent::TpsUpdated {
                endpoint_id,
                model_id,
                tps,
                output_tokens: output_tokens as u32,
                duration_ms,
            });
        }
    });
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
        let error_body = match response.text().await {
            Ok(body) => body,
            Err(e) => {
                tracing::debug!(
                    "Failed to read error body from endpoint {}: {}",
                    endpoint.name,
                    e
                );
                String::new()
            }
        };
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
