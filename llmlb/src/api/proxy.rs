//! LLM runtimeプロキシ APIハンドラー
//!
//! # SPEC-f8e3a1b7: Endpoint型への移行完了
//!
//! このモジュールはEndpoint型を使用しています。

use crate::api::openai_util::classify_upstream_request_error;
use crate::balancer::WaitResult;
use crate::common::{
    error::LbError,
    protocol::{RequestResponseRecord, TpsApiKind},
};
use crate::token::StreamingTokenAccumulator;
use crate::{
    types::endpoint::{Endpoint, SupportedAPI},
    AppState,
};
use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, StatusCode},
    response::Response,
};
use futures::{Stream, StreamExt, TryStreamExt};
use std::{io, pin::Pin, sync::Arc, time::Instant};

/// ストリーミング中継のアイドル（無応答）タイムアウト（Issue #978）
///
/// 上流からチャンクがこの時間届かなければ、詰まった接続として中継を
/// 打ち切る。正常な生成ではチャンクは継続的に流れるため、完全な無応答が
/// 120 秒続く状態は「生成中」ではなく「詰まり」と判定する。沈黙が起こる
/// 主な局面は初回チャンク前のプリフィルであり、初回チャンク待ちもこの
/// 対象に含める。ストリーミングには総量タイムアウトの上限を設けず
/// （オーナー決定 2026-11-26）、時間管理はこのアイドルタイムアウトのみが
/// 担う。総量タイムアウト（`inference_timeout_secs`、既定 600 秒）は
/// 非ストリーミング経路の絶対上限としてのみ使う。
pub(crate) const STREAM_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// キュー待機の上限時間（Issue #977）
///
/// 全エンドポイントが実行中の場合に空きを待つ上限。推論タイムアウト
/// （120〜600秒）に対して短く抑えた機会的な平準化であり、タイムアウト
/// した場合は従来どおり即時選択へフォールバックする。
const QUEUE_WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// キュー待機者の上限数（Issue #977）
///
/// これを超えたリクエストは待機せず即時選択へフォールバックし、
/// llmlb 側に待機が無制限に積み上がらないようにする。
const MAX_QUEUE_WAITERS: usize = 100;

/// キュー付きエンドポイント選択の結果
pub(crate) enum QueueSelection {
    /// エンドポイントが見つかった
    Ready {
        endpoint: Box<Endpoint>,
        queued_wait_ms: Option<u128>,
    },
}

/// モデル対応エンドポイントにアイドルが無ければ、空きが出るまで上限時間だけ待機する（Issue #977）。
///
/// 戻り値は実際に待機したミリ秒数。待機が不要だった場合（アイドルあり）と、
/// 待機者上限の超過で待機しなかった場合は `None` を返す。タイムアウトした場合も
/// 待機実績として `Some` を返すが、呼び出し側はそのまま選択へ進むため、
/// 空きが来なくてもリクエストは従来どおり進む（可用性を落とさない）。
async fn wait_for_idle_slot_for_model(state: &AppState, model_id: &str) -> Option<u128> {
    let load_manager = &state.balancer.load_manager;
    if load_manager.has_idle_endpoint_for_model(model_id).await {
        return None;
    }

    let started = Instant::now();
    match load_manager
        .wait_for_idle_node_with_timeout_for_model(model_id, MAX_QUEUE_WAITERS, QUEUE_WAIT_TIMEOUT)
        .await
    {
        WaitResult::Ready => {
            let waited_ms = started.elapsed().as_millis();
            tracing::debug!(
                model = %model_id,
                waited_ms,
                "Queue wait finished: an endpoint became idle"
            );
            Some(waited_ms)
        }
        WaitResult::Timeout => {
            let waited_ms = started.elapsed().as_millis();
            tracing::debug!(
                model = %model_id,
                waited_ms,
                "Queue wait timed out: falling back to immediate selection"
            );
            Some(waited_ms)
        }
        WaitResult::CapacityExceeded => None,
    }
}

/// モデル対応のエンドポイントをTPS優先・キュー付きで選択
pub(crate) async fn select_available_endpoint_with_queue_for_model(
    state: &AppState,
    model_id: &str,
    api_kind: Option<TpsApiKind>,
) -> Result<QueueSelection, LbError> {
    // 全エンドポイントが実行中なら、選択の前に空きを待つ（Issue #977）
    let queued_wait_ms = wait_for_idle_slot_for_model(state, model_id).await;

    let endpoint = state
        .balancer
        .load_manager
        .select_endpoint_by_tps_ready_for_model(model_id, api_kind)
        .await?;

    tracing::debug!(
        model = %model_id,
        endpoint_id = %endpoint.id,
        endpoint_name = %endpoint.name,
        ?api_kind,
        "Selected ready endpoint by TPS priority"
    );

    Ok(QueueSelection::Ready {
        endpoint: Box::new(endpoint),
        queued_wait_ms,
    })
}

/// フェイルオーバーの最大試行回数（初回送信を含む総試行回数、Issue #976）
///
/// 1回の推論リクエストで試行するエンドポイント数の上限。候補がこれより
/// 多くても上限までしか試行せず、無制限の再試行はしない。
pub(crate) const MAX_FAILOVER_ATTEMPTS: usize = 3;

/// 上流の失敗ステータスがフェイルオーバー（別エンドポイントへの再試行）の
/// 対象かどうか（Issue #976）
///
/// 429（上流側の容量・レート制限）と 5xx（上流側の障害）は、別エンドポイント
/// では成功する可能性があるため再試行する。それ以外の 4xx はクライアント
/// 起因であり、別エンドポイントでも同じ結果になるため再試行しない
/// （404 の hop-off は #964 の領分としてここでは扱わない）。
pub(crate) fn is_failover_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

/// モデル対応エンドポイントをTPS優先の順序付き候補列で選択（フェイルオーバー用）
///
/// 候補列は最大 `MAX_FAILOVER_ATTEMPTS` 件に切り詰める。
pub(crate) async fn select_endpoint_candidates_with_queue_for_model(
    state: &AppState,
    model_id: &str,
    api_kind: Option<TpsApiKind>,
) -> Result<Vec<Endpoint>, LbError> {
    let mut endpoints = state
        .balancer
        .load_manager
        .select_endpoints_by_tps_ready_for_model(model_id, api_kind)
        .await?;
    endpoints.truncate(MAX_FAILOVER_ATTEMPTS);

    tracing::debug!(
        model = %model_id,
        candidates = endpoints.len(),
        ?api_kind,
        "Selected ordered endpoint candidates by TPS priority"
    );

    Ok(endpoints)
}

/// モデルと必須APIに対応するエンドポイントをTPS優先の順序付き候補列で選択
/// （フェイルオーバー用）
///
/// 候補列は最大 `MAX_FAILOVER_ATTEMPTS` 件に切り詰める。
pub(crate) async fn select_endpoint_candidates_with_queue_for_model_and_api(
    state: &AppState,
    model_id: &str,
    required_api: SupportedAPI,
    api_kind: Option<TpsApiKind>,
) -> Result<Vec<Endpoint>, LbError> {
    let endpoints = state
        .balancer
        .endpoint_registry
        .find_by_model_and_supported_api(model_id, required_api)
        .await;
    if endpoints.is_empty() {
        return Err(LbError::NoCapableEndpoints(model_id.to_string()));
    }

    let mut endpoints = state
        .balancer
        .load_manager
        .select_endpoints_by_tps_ready_from_candidates(endpoints, model_id, api_kind)
        .await?;
    endpoints.truncate(MAX_FAILOVER_ATTEMPTS);

    tracing::debug!(
        model = %model_id,
        candidates = endpoints.len(),
        required_api = %required_api,
        ?api_kind,
        "Selected ordered endpoint candidates by TPS priority and required API"
    );

    Ok(endpoints)
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
    api_kind: Option<TpsApiKind>,
    endpoint_type: crate::types::endpoint::EndpointType,
    request_started_at: Instant,
    endpoint_registry: crate::registry::endpoints::EndpointRegistry,
    load_manager: crate::balancer::LoadManager,
    event_bus: crate::events::SharedEventBus,
    request_lease: Option<crate::balancer::RequestLease>,
    idle_timeout: std::time::Duration,
) -> Result<Response, LbError> {
    struct TpsTrackingState {
        upstream: Pin<Box<dyn Stream<Item = Result<axum::body::Bytes, reqwest::Error>> + Send>>,
        accumulator: StreamingTokenAccumulator,
        sse_buffer: String,
        endpoint_id: uuid::Uuid,
        model_id: String,
        api_kind: Option<TpsApiKind>,
        endpoint_type: crate::types::endpoint::EndpointType,
        request_started_at: Instant,
        endpoint_registry: crate::registry::endpoints::EndpointRegistry,
        load_manager: crate::balancer::LoadManager,
        event_bus: crate::events::SharedEventBus,
        stats_recorded: bool,
        // ストリーム完走時に実時間で完了する RequestLease。
        // ヘッダー受信時点で早期解放すると、ストリーミング中の assigned_active が
        // 過小になり、レイテンシも過小評価され、idle 通知も早まる（lease-ttfb）。
        request_lease: Option<crate::balancer::RequestLease>,
        // チャンク無受信の許容時間（Issue #978）。超過したら無応答として打ち切る。
        idle_timeout: std::time::Duration,
    }

    impl TpsTrackingState {
        fn finalize_output_tokens_and_duration(&mut self) -> (u64, u64) {
            if !self.sse_buffer.is_empty() {
                let pending = std::mem::take(&mut self.sse_buffer);
                self.accumulator
                    .process_chunk(pending.trim_end_matches('\r'));
            }

            let usage = self.accumulator.finalize();
            let output_tokens = usage.output_tokens.unwrap_or(0) as u64;
            let duration_ms = if output_tokens > 0 {
                self.request_started_at.elapsed().as_millis().max(1) as u64
            } else {
                0
            };

            (output_tokens, duration_ms)
        }

        fn record_stats_once(&mut self, success: bool, output_tokens: u64, duration_ms: u64) {
            if self.stats_recorded {
                return;
            }
            self.stats_recorded = true;

            record_endpoint_request_stats(
                self.endpoint_registry.clone(),
                self.endpoint_id,
                self.model_id.clone(),
                success,
                output_tokens,
                duration_ms,
                self.api_kind,
                self.endpoint_type,
                self.load_manager.clone(),
                self.event_bus.clone(),
            );
        }

        /// ストリーム終端で RequestLease を実時間で完了する。
        ///
        /// 成功時はストリーム完走までの実時間で推論レイテンシも更新する
        /// （ヘッダー受信時点の過小なレイテンシではなく）。
        /// 明示的に完了しなかった場合（クライアント切断等）は lease の Drop が
        /// Error 扱いで自動完了するため、カウンタ残留は起きない。
        async fn finish_lease(&mut self, outcome: crate::balancer::RequestOutcome) {
            let full = self.request_started_at.elapsed();
            if matches!(outcome, crate::balancer::RequestOutcome::Success) {
                if let Err(err) = self
                    .endpoint_registry
                    .update_inference_latency(self.endpoint_id, full.as_millis() as f64)
                    .await
                {
                    tracing::debug!(
                        endpoint_id = %self.endpoint_id,
                        error = %err,
                        "Failed to update inference latency at stream end"
                    );
                }
            }
            if let Some(lease) = self.request_lease.take() {
                if let Err(err) = lease.complete(outcome, full).await {
                    tracing::warn!(
                        endpoint_id = %self.endpoint_id,
                        error = %err,
                        "Failed to complete streaming request lease"
                    );
                }
            }
        }
    }

    impl Drop for TpsTrackingState {
        fn drop(&mut self) {
            if self.stats_recorded {
                return;
            }

            let (output_tokens, duration_ms) = self.finalize_output_tokens_and_duration();

            if tokio::runtime::Handle::try_current().is_ok() {
                self.record_stats_once(true, output_tokens, duration_ms);
            } else {
                tracing::warn!(
                    endpoint_id = %self.endpoint_id,
                    model_id = %self.model_id,
                    "Streaming TPS tracker dropped without runtime; skipping stats fallback"
                );
            }
        }
    }

    let status = response.status();
    let headers = response.headers().clone();

    let state = TpsTrackingState {
        upstream: Box::pin(response.bytes_stream()),
        accumulator: StreamingTokenAccumulator::new(&model_id),
        sse_buffer: String::new(),
        endpoint_id,
        model_id,
        api_kind,
        endpoint_type,
        request_started_at,
        endpoint_registry,
        load_manager,
        event_bus,
        stats_recorded: false,
        request_lease,
        idle_timeout,
    };

    let tracked_stream = futures::stream::try_unfold(state, |mut state| async move {
        // チャンク待ちにアイドルタイムアウトを適用する（Issue #978）。
        // 初回チャンク待ちも対象で、上流がヘッダー後に無応答になった場合は
        // 総量タイムアウトを待たず、無応答として失敗扱いで打ち切る。
        match tokio::time::timeout(state.idle_timeout, state.upstream.next()).await {
            Ok(Some(Ok(chunk))) => {
                let chunk_text = String::from_utf8_lossy(chunk.as_ref());
                process_sse_lines(&mut state.sse_buffer, &chunk_text, &mut state.accumulator);
                Ok(Some((chunk, state)))
            }
            Ok(Some(Err(err))) => {
                state.record_stats_once(false, 0, 0);
                state
                    .finish_lease(crate::balancer::RequestOutcome::Error)
                    .await;
                Err(io::Error::other(err))
            }
            Ok(None) => {
                let (output_tokens, duration_ms) = state.finalize_output_tokens_and_duration();
                state.record_stats_once(true, output_tokens, duration_ms);
                state
                    .finish_lease(crate::balancer::RequestOutcome::Success)
                    .await;
                Ok(None)
            }
            Err(_elapsed) => {
                tracing::warn!(
                    endpoint_id = %state.endpoint_id,
                    model_id = %state.model_id,
                    idle_timeout_secs = state.idle_timeout.as_secs(),
                    "Streaming relay aborted: upstream idle timeout"
                );
                state.record_stats_once(false, 0, 0);
                state
                    .finish_lease(crate::balancer::RequestOutcome::Error)
                    .await;
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "upstream stream idle timeout",
                ))
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
    endpoint_registry: crate::registry::endpoints::EndpointRegistry,
    endpoint_id: uuid::Uuid,
    model_id: String,
    success: bool,
    output_tokens: u64,
    duration_ms: u64,
    api_kind: Option<TpsApiKind>,
    endpoint_type: crate::types::endpoint::EndpointType,
    load_manager: crate::balancer::LoadManager,
    event_bus: crate::events::SharedEventBus,
) {
    tokio::spawn(async move {
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let pool = endpoint_registry.pool().clone();

        if let Err(e) = endpoint_registry
            .increment_request_counters(endpoint_id, success)
            .await
        {
            tracing::error!("Failed to increment endpoint request counters: {}", e);
        }

        // Issue #975: 推論リクエストの成否をエンドポイントステータスの判定へ反映する。
        // 成功で連続失敗をリセットし、失敗が連続すると Error / Offline へ遷移して
        // 選択から外れる（ヘルスチェックだけでは推論経路の詰まりを検知できないため）。
        if success {
            endpoint_registry.record_request_success(endpoint_id).await;
        } else {
            match endpoint_registry
                .record_request_failure(endpoint_id, "Inference request failed")
                .await
            {
                Ok(true) => {
                    // ヘルスチェック側の遷移時と同様に TPS をリセットする
                    load_manager.clear_tps_for_endpoint(endpoint_id).await;
                }
                Ok(false) => {}
                Err(e) => {
                    tracing::error!("Failed to record endpoint request failure: {}", e);
                }
            }
        }

        // TPS計測対象かつ成功かつ有効トークンがある場合のみトークン・時間をDB永続化
        let should_update_tps = endpoint_type.is_tps_trackable()
            && api_kind.is_some()
            && success
            && output_tokens > 0
            && duration_ms > 0;
        let (tokens, duration) = if should_update_tps {
            (output_tokens, duration_ms)
        } else {
            (0, 0)
        };

        let api_kind_str = api_kind
            .and_then(|k| serde_json::to_value(k).ok())
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "chat_completions".to_string());

        if let Err(e) = crate::db::endpoint_daily_stats::upsert_daily_stats_with_api_kind(
            &pool,
            endpoint_id,
            &model_id,
            &date,
            &api_kind_str,
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
            let api_kind = api_kind.expect("checked above");
            load_manager
                .update_tps(
                    endpoint_id,
                    model_id.clone(),
                    api_kind,
                    output_tokens,
                    duration_ms,
                )
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

        // SPEC #582 FR-048c: リクエスト数・日次統計・TPS の確定を購読者へ通知する。
        // リクエストごとに発生するため集約して発行する（FR-048d）。llmlb はエンドポイント内部の
        // リソース使用率を観測しないので、値なしで配信する。
        event_bus.publish_coalesced(crate::events::DashboardEvent::MetricsUpdated {
            runtime_id: endpoint_id,
            cpu_usage: None,
            memory_usage: None,
            gpu_usage: None,
        });
    });
}
/// エンドポイントにリクエストを転送
///
/// OpenAI互換APIエンドポイントにリクエストを転送し、レスポンスを返す。
///
/// タイムアウト方針（Issue #978、オーナー決定 2026-11-26）: 総量タイムアウト
/// （`inference_timeout_secs`）は非ストリーミングにのみ付与する。reqwest の
/// per-request timeout は応答ボディの読み取りにも効くため、ストリーミングに
/// 付与すると正当に時間のかかる生成が総量で切れてしまう。ストリーミングの
/// 無応答検出は、中継側のアイドルタイムアウト（`STREAM_IDLE_TIMEOUT`）が担う。
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
        .body(body);

    if !stream {
        request_builder = request_builder.timeout(std::time::Duration::from_secs(
            endpoint.inference_timeout_secs as u64,
        ));
    }

    // APIキーがあれば追加
    if let Some(api_key) = &endpoint.api_key {
        request_builder = request_builder.bearer_auth(api_key);
    }

    // 上流のレスポンスは非2xxでもそのまま返し、ステータス処理は呼び出し側に委ねる。
    // （全呼び出し元が上流のステータス/本文を保持して扱うため、ここでの非2xx→Err化は不要）
    request_builder.send().await.map_err(|e| {
        tracing::error!(
            "Failed to forward request to endpoint {}: {}",
            endpoint.name,
            e
        );
        let classified =
            classify_upstream_request_error(&e, &url, endpoint.inference_timeout_secs, None);
        if classified.status_code == StatusCode::GATEWAY_TIMEOUT {
            LbError::Timeout(classified.record_message)
        } else {
            LbError::Http(classified.record_message)
        }
    })
}

// NOTE: テストはNodeRegistry廃止に伴い削除されました。
// 新しいテストはEndpointRegistryベースで tests/integration/ に追加してください。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::StreamingTokenAccumulator;

    // --- QueueSelection enum ---

    #[test]
    fn queue_selection_ready_variant_holds_endpoint_and_wait() {
        let ep = Endpoint::new(
            "ep1".to_string(),
            "http://localhost:8080".to_string(),
            crate::types::endpoint::EndpointType::Xllm,
        );
        let qs = QueueSelection::Ready {
            endpoint: Box::new(ep),
            queued_wait_ms: Some(42),
        };
        let QueueSelection::Ready {
            endpoint,
            queued_wait_ms,
        } = qs;
        assert_eq!(endpoint.name, "ep1");
        assert_eq!(queued_wait_ms, Some(42));
    }

    #[test]
    fn queue_selection_ready_variant_no_wait() {
        let ep = Endpoint::new(
            "ep2".to_string(),
            "http://localhost:8081".to_string(),
            crate::types::endpoint::EndpointType::Ollama,
        );
        let qs = QueueSelection::Ready {
            endpoint: Box::new(ep),
            queued_wait_ms: None,
        };
        let QueueSelection::Ready { queued_wait_ms, .. } = qs;
        assert!(queued_wait_ms.is_none());
    }

    // --- Issue #976: is_failover_retryable_status ---

    #[test]
    fn failover_retryable_for_server_errors() {
        for status in [
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::BAD_GATEWAY,
            StatusCode::SERVICE_UNAVAILABLE,
            StatusCode::GATEWAY_TIMEOUT,
        ] {
            assert!(
                is_failover_retryable_status(status),
                "expected {status} to be failover-retryable"
            );
        }
    }

    #[test]
    fn failover_retryable_for_too_many_requests() {
        // 429 は上流側の容量都合であり、別エンドポイントでは成功しうる
        assert!(is_failover_retryable_status(StatusCode::TOO_MANY_REQUESTS));
    }

    #[test]
    fn failover_not_retryable_for_client_errors() {
        for status in [
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
            StatusCode::UNPROCESSABLE_ENTITY,
        ] {
            assert!(
                !is_failover_retryable_status(status),
                "expected {status} to not be failover-retryable"
            );
        }
    }

    #[test]
    fn failover_not_retryable_for_success() {
        assert!(!is_failover_retryable_status(StatusCode::OK));
        assert!(!is_failover_retryable_status(StatusCode::CREATED));
    }

    // --- process_sse_lines ---

    #[test]
    fn process_sse_lines_empty_chunk() {
        let mut buffer = String::new();
        let mut acc = StreamingTokenAccumulator::new("test-model");
        process_sse_lines(&mut buffer, "", &mut acc);
        assert!(buffer.is_empty());
    }

    #[test]
    fn process_sse_lines_single_line_with_newline() {
        let mut buffer = String::new();
        let mut acc = StreamingTokenAccumulator::new("test-model");
        process_sse_lines(
            &mut buffer,
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n",
            &mut acc,
        );
        assert!(buffer.is_empty());
    }

    #[test]
    fn process_sse_lines_partial_line_no_newline() {
        let mut buffer = String::new();
        let mut acc = StreamingTokenAccumulator::new("test-model");
        process_sse_lines(&mut buffer, "data: partial", &mut acc);
        assert_eq!(buffer, "data: partial");
    }

    #[test]
    fn process_sse_lines_multiple_lines_in_one_chunk() {
        let mut buffer = String::new();
        let mut acc = StreamingTokenAccumulator::new("test-model");
        process_sse_lines(&mut buffer, "data: line1\ndata: line2\n", &mut acc);
        assert!(buffer.is_empty());
    }

    #[test]
    fn process_sse_lines_split_across_chunks() {
        let mut buffer = String::new();
        let mut acc = StreamingTokenAccumulator::new("test-model");

        // First chunk: partial line
        process_sse_lines(&mut buffer, "data: hel", &mut acc);
        assert_eq!(buffer, "data: hel");

        // Second chunk: rest of line
        process_sse_lines(&mut buffer, "lo\n", &mut acc);
        assert!(buffer.is_empty());
    }

    #[test]
    fn process_sse_lines_carriage_return_stripped() {
        let mut buffer = String::new();
        let mut acc = StreamingTokenAccumulator::new("test-model");
        process_sse_lines(&mut buffer, "data: test\r\n", &mut acc);
        assert!(buffer.is_empty());
    }

    #[test]
    fn process_sse_lines_done_marker() {
        let mut buffer = String::new();
        let mut acc = StreamingTokenAccumulator::new("test-model");
        process_sse_lines(&mut buffer, "data: [DONE]\n", &mut acc);
        assert!(buffer.is_empty());
    }

    #[test]
    fn process_sse_lines_buffer_accumulates_partial() {
        let mut buffer = String::new();
        let mut acc = StreamingTokenAccumulator::new("test-model");

        process_sse_lines(&mut buffer, "abc", &mut acc);
        assert_eq!(buffer, "abc");

        process_sse_lines(&mut buffer, "def", &mut acc);
        assert_eq!(buffer, "abcdef");

        process_sse_lines(&mut buffer, "\n", &mut acc);
        assert!(buffer.is_empty());
    }

    #[test]
    fn process_sse_lines_empty_lines() {
        let mut buffer = String::new();
        let mut acc = StreamingTokenAccumulator::new("test-model");
        process_sse_lines(&mut buffer, "\n\n\n", &mut acc);
        assert!(buffer.is_empty());
    }

    #[test]
    fn process_sse_lines_mixed_content() {
        let mut buffer = String::new();
        let mut acc = StreamingTokenAccumulator::new("test-model");
        process_sse_lines(
            &mut buffer,
            "data: {\"choices\":[{\"delta\":{\"content\":\"A\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"B\"}}]}\nremaining",
            &mut acc,
        );
        assert_eq!(buffer, "remaining");
    }

    // --- forward_to_endpoint URL construction ---

    #[test]
    fn forward_url_trims_trailing_slash() {
        let ep = Endpoint::new(
            "ep".to_string(),
            "http://localhost:8080/".to_string(),
            crate::types::endpoint::EndpointType::Xllm,
        );
        let url = format!(
            "{}{}",
            ep.base_url.trim_end_matches('/'),
            "/v1/chat/completions"
        );
        assert_eq!(url, "http://localhost:8080/v1/chat/completions");
    }

    #[test]
    fn forward_url_no_trailing_slash() {
        let ep = Endpoint::new(
            "ep".to_string(),
            "http://10.0.0.1:11434".to_string(),
            crate::types::endpoint::EndpointType::Ollama,
        );
        let url = format!("{}{}", ep.base_url.trim_end_matches('/'), "/v1/completions");
        assert_eq!(url, "http://10.0.0.1:11434/v1/completions");
    }

    #[test]
    fn forward_url_multiple_trailing_slashes() {
        let base_url = "http://localhost:8080///";
        let url = format!("{}{}", base_url.trim_end_matches('/'), "/v1/embeddings");
        assert_eq!(url, "http://localhost:8080/v1/embeddings");
    }

    // --- Endpoint bearer_auth logic ---

    #[test]
    fn endpoint_with_api_key_has_some() {
        let mut ep = Endpoint::new(
            "ep".to_string(),
            "http://localhost:8080".to_string(),
            crate::types::endpoint::EndpointType::OpenaiCompatible,
        );
        ep.api_key = Some("sk-test-key".to_string());
        assert!(ep.api_key.is_some());
    }

    #[test]
    fn endpoint_without_api_key_has_none() {
        let ep = Endpoint::new(
            "ep".to_string(),
            "http://localhost:8080".to_string(),
            crate::types::endpoint::EndpointType::OpenaiCompatible,
        );
        assert!(ep.api_key.is_none());
    }

    // --- LbError::Http construction from forward_to_endpoint ---

    #[test]
    fn lb_error_http_format() {
        let err = LbError::Http("Endpoint request failed: connection refused".to_string());
        assert!(err.to_string().contains("Endpoint request failed"));
    }

    // --- forward_streaming_response content-type behavior ---

    #[tokio::test]
    async fn forward_streaming_response_sets_json_content_type() {
        // Create a minimal reqwest response
        let response = axum::http::Response::builder()
            .status(200)
            .header("x-custom", "test")
            .body("test body")
            .unwrap();
        let reqwest_response = reqwest::Response::from(response);

        let axum_response = forward_streaming_response(reqwest_response).unwrap();
        assert_eq!(axum_response.status(), StatusCode::OK);
        assert_eq!(
            axum_response
                .headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "application/json"
        );
    }

    #[tokio::test]
    async fn forward_streaming_response_preserves_sse_content_type() {
        let response = axum::http::Response::builder()
            .status(200)
            .header("content-type", "text/event-stream")
            .body("data: test\n\n")
            .unwrap();
        let reqwest_response = reqwest::Response::from(response);

        let axum_response = forward_streaming_response(reqwest_response).unwrap();
        assert_eq!(
            axum_response
                .headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "text/event-stream"
        );
    }

    #[tokio::test]
    async fn forward_streaming_response_maps_status_code() {
        let response = axum::http::Response::builder()
            .status(201)
            .body("")
            .unwrap();
        let reqwest_response = reqwest::Response::from(response);

        let axum_response = forward_streaming_response(reqwest_response).unwrap();
        assert_eq!(axum_response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn forward_streaming_response_maps_error_status() {
        let response = axum::http::Response::builder()
            .status(500)
            .body("")
            .unwrap();
        let reqwest_response = reqwest::Response::from(response);

        let axum_response = forward_streaming_response(reqwest_response).unwrap();
        assert_eq!(axum_response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn forward_streaming_response_preserves_custom_headers() {
        let response = axum::http::Response::builder()
            .status(200)
            .header("x-request-id", "abc123")
            .body("")
            .unwrap();
        let reqwest_response = reqwest::Response::from(response);

        let axum_response = forward_streaming_response(reqwest_response).unwrap();
        assert_eq!(
            axum_response
                .headers()
                .get("x-request-id")
                .unwrap()
                .to_str()
                .unwrap(),
            "abc123"
        );
    }

    #[tokio::test]
    async fn forward_streaming_response_maps_404() {
        let response = axum::http::Response::builder()
            .status(404)
            .body("")
            .unwrap();
        let reqwest_response = reqwest::Response::from(response);

        let axum_response = forward_streaming_response(reqwest_response).unwrap();
        assert_eq!(axum_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn forward_streaming_response_maps_429() {
        let response = axum::http::Response::builder()
            .status(429)
            .body("")
            .unwrap();
        let reqwest_response = reqwest::Response::from(response);

        let axum_response = forward_streaming_response(reqwest_response).unwrap();
        assert_eq!(axum_response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    // --- lease-ttfb: ストリーミング forwarder は配信完了まで lease を保持する ---

    #[tokio::test]
    async fn streaming_forwarder_holds_lease_until_stream_consumed() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create test db");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");
        let registry = crate::registry::endpoints::EndpointRegistry::new(pool)
            .await
            .expect("create registry");
        let endpoint = Endpoint::new(
            "lease-stream-ep".to_string(),
            "http://localhost:1".to_string(),
            crate::types::endpoint::EndpointType::OpenaiCompatible,
        );
        let endpoint_id = endpoint.id;
        registry.add(endpoint).await.expect("add endpoint");
        let load_manager = crate::balancer::LoadManager::new(Arc::new(registry.clone()));
        let event_bus = crate::events::create_shared_event_bus();

        let lease = load_manager
            .begin_request(endpoint_id)
            .await
            .expect("begin request");
        assert_eq!(
            load_manager
                .snapshot(endpoint_id)
                .await
                .unwrap()
                .active_requests,
            1,
            "begin_request must mark one active request"
        );

        let response = reqwest::Response::from(
            axum::http::Response::builder()
                .status(200)
                .header("content-type", "text/event-stream")
                .body("data: {\"choices\":[]}\n\ndata: [DONE]\n\n")
                .unwrap(),
        );

        let axum_response = forward_streaming_response_with_tps_tracking(
            response,
            endpoint_id,
            "m".to_string(),
            None,
            crate::types::endpoint::EndpointType::OpenaiCompatible,
            Instant::now(),
            registry,
            load_manager.clone(),
            event_bus,
            Some(lease),
            STREAM_IDLE_TIMEOUT,
        )
        .expect("forward streaming");

        // ストリーム未消費の段階では lease は保持されたまま（早期解放しない）
        assert_eq!(
            load_manager
                .snapshot(endpoint_id)
                .await
                .unwrap()
                .active_requests,
            1,
            "lease must be held until the stream is consumed"
        );

        // ボディを完全に消費するとストリーム終端で lease が完了する
        let _ = axum::body::to_bytes(axum_response.into_body(), usize::MAX)
            .await
            .expect("consume body");

        assert_eq!(
            load_manager
                .snapshot(endpoint_id)
                .await
                .unwrap()
                .active_requests,
            0,
            "lease must be completed after the stream is fully consumed"
        );
    }

    // --- Issue #978: ストリーミング中継のタイムアウト方針テストは
    // proxy/tests/streaming_timeouts.rs へ分離（FR-009 の行数上限対応） ---
    mod streaming_timeouts;

    // --- Timeout configuration in forward_to_endpoint ---

    #[test]
    fn endpoint_inference_timeout_default() {
        let ep = Endpoint::new(
            "ep".to_string(),
            "http://localhost:8080".to_string(),
            crate::types::endpoint::EndpointType::Xllm,
        );
        assert_eq!(ep.inference_timeout_secs, 600);
    }

    #[test]
    fn endpoint_inference_timeout_custom() {
        let mut ep = Endpoint::new(
            "ep".to_string(),
            "http://localhost:8080".to_string(),
            crate::types::endpoint::EndpointType::Xllm,
        );
        ep.inference_timeout_secs = 60;
        assert_eq!(
            std::time::Duration::from_secs(ep.inference_timeout_secs as u64),
            std::time::Duration::from_secs(60)
        );
    }

    // --- Issue #977: キュー待機の接続（select_available_endpoint_with_queue_for_model） ---

    async fn create_test_state() -> AppState {
        use crate::audit::writer::{AuditLogWriter, AuditLogWriterConfig};
        use crate::db::audit_log::AuditLogStorage;

        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create test db");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");

        let request_history = Arc::new(crate::db::request_history::RequestHistoryStorage::new(
            pool.clone(),
        ));
        let endpoint_registry = crate::registry::endpoints::EndpointRegistry::new(pool.clone())
            .await
            .expect("create endpoint registry");
        let load_manager = crate::balancer::LoadManager::new(Arc::new(endpoint_registry.clone()));
        let http_client = reqwest::Client::new();
        let inference_gate = crate::inference_gate::InferenceGate::default();
        let shutdown = crate::shutdown::ShutdownController::default();
        let update_manager = crate::update::UpdateManager::new(
            http_client.clone(),
            inference_gate.clone(),
            shutdown.clone(),
        )
        .expect("create update manager");
        let audit_log_storage = Arc::new(AuditLogStorage::new(pool.clone()));
        let audit_log_writer = AuditLogWriter::new(
            AuditLogStorage::new(pool.clone()),
            AuditLogWriterConfig {
                flush_interval_secs: 300,
                buffer_capacity: 100,
                batch_interval_secs: 300,
            },
        );

        AppState {
            balancer: crate::BalancerState {
                load_manager,
                endpoint_registry,
                request_history,
            },
            db_pool: pool,
            auth: crate::AuthState {
                jwt_secret: "test-secret".to_string(),
            },
            http_client,
            event_bus: crate::events::create_shared_event_bus(),
            lifecycle: crate::LifecycleState {
                inference_gate,
                shutdown,
                update_manager,
            },
            audit: crate::AuditState {
                writer: audit_log_writer,
                storage: audit_log_storage,
                archive_pool: None,
            },
        }
    }

    async fn add_online_endpoint_with_model(
        state: &AppState,
        name: &str,
        model_id: &str,
    ) -> Endpoint {
        let mut endpoint = Endpoint::new(
            name.to_string(),
            "http://localhost:18080".to_string(),
            crate::types::endpoint::EndpointType::OpenaiCompatible,
        );
        endpoint.status = crate::types::endpoint::EndpointStatus::Online;
        let endpoint_id = endpoint.id;
        state
            .balancer
            .endpoint_registry
            .add(endpoint.clone())
            .await
            .expect("add endpoint");
        state
            .balancer
            .endpoint_registry
            .add_model(&crate::types::endpoint::EndpointModel {
                endpoint_id,
                model_id: model_id.to_string(),
                capabilities: None,
                max_tokens: None,
                last_checked: None,
                supported_apis: vec![SupportedAPI::ChatCompletions],
                canonical_name: None,
            })
            .await
            .expect("add endpoint model");
        endpoint
    }

    #[tokio::test]
    async fn select_with_queue_returns_immediately_when_endpoint_idle() {
        let _lock = crate::db::test_utils::TEST_LOCK.lock().await;
        let state = create_test_state().await;
        let endpoint = add_online_endpoint_with_model(&state, "idle-ep", "queue-model").await;

        let started = std::time::Instant::now();
        let selection = select_available_endpoint_with_queue_for_model(&state, "queue-model", None)
            .await
            .expect("selection should succeed");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "アイドルなエンドポイントがある場合は待機しない"
        );

        let QueueSelection::Ready {
            endpoint: selected,
            queued_wait_ms,
        } = selection;
        assert_eq!(selected.id, endpoint.id);
        assert!(
            queued_wait_ms.is_none(),
            "待機していない場合は queued_wait_ms は None"
        );
    }

    #[tokio::test]
    async fn select_with_queue_waits_until_busy_endpoint_frees() {
        let _lock = crate::db::test_utils::TEST_LOCK.lock().await;
        let state = create_test_state().await;
        let endpoint = add_online_endpoint_with_model(&state, "busy-ep", "queue-model").await;
        let lease = state
            .balancer
            .load_manager
            .begin_request(endpoint.id)
            .await
            .expect("lease should start");

        let select_state = state.clone();
        let select_task = tokio::spawn(async move {
            select_available_endpoint_with_queue_for_model(&select_state, "queue-model", None).await
        });

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(
            !select_task.is_finished(),
            "全エンドポイントが実行中の間は選択がキューで待機するべき"
        );

        lease
            .complete(
                crate::balancer::RequestOutcome::Success,
                std::time::Duration::from_millis(1),
            )
            .await
            .expect("lease should complete");

        let selection = select_task
            .await
            .expect("select task should finish")
            .expect("selection should succeed");
        let QueueSelection::Ready {
            endpoint: selected,
            queued_wait_ms,
        } = selection;
        assert_eq!(selected.id, endpoint.id);
        let wait_ms = queued_wait_ms.expect("待機した場合は queued_wait_ms が報告されるべき");
        assert!(
            wait_ms >= 100,
            "実測の待機時間が報告されるべき (got {wait_ms}ms)"
        );
    }

    #[tokio::test]
    async fn select_with_queue_falls_back_after_timeout_when_never_free() {
        let _lock = crate::db::test_utils::TEST_LOCK.lock().await;
        let state = create_test_state().await;
        let endpoint = add_online_endpoint_with_model(&state, "stuck-ep", "queue-model").await;
        let _lease = state
            .balancer
            .load_manager
            .begin_request(endpoint.id)
            .await
            .expect("lease should start");

        // 空きが永遠に来なくても、待機上限を超えたら従来どおり即時選択へ
        // フォールバックしてリクエストを進める（可用性を落とさない）。
        let started = std::time::Instant::now();
        let selection = select_available_endpoint_with_queue_for_model(&state, "queue-model", None)
            .await
            .expect("selection should succeed after queue timeout");
        let elapsed = started.elapsed();
        assert!(
            elapsed >= QUEUE_WAIT_TIMEOUT,
            "待機上限までは待つべき (elapsed {elapsed:?})"
        );
        assert!(
            elapsed < QUEUE_WAIT_TIMEOUT + std::time::Duration::from_secs(3),
            "待機上限を超えて待たないべき (elapsed {elapsed:?})"
        );

        let QueueSelection::Ready {
            endpoint: selected,
            queued_wait_ms,
        } = selection;
        assert_eq!(selected.id, endpoint.id);
        assert!(
            queued_wait_ms.is_some(),
            "タイムアウト時も実際に待った時間は報告されるべき"
        );
    }
}
