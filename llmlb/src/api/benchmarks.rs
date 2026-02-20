//! Benchmark APIs for comparable TPS measurement.
//!
//! 本番TPS（production）は監視向けに残し、比較可能な指標は固定シナリオの
//! ベンチ実行で分離して取得する。

use super::error::AppError;
use crate::common::{
    error::{CommonError, LbError},
    protocol::{TpsApiKind, TpsSource},
};
use crate::{token::extract_usage_from_response, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::LazyLock, time::Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

const DEFAULT_TOTAL_REQUESTS: u32 = 20;
const DEFAULT_CONCURRENCY: u16 = 4;
const DEFAULT_MAX_TOKENS: u32 = 128;
const DEFAULT_TEMPERATURE: f32 = 0.2;
const MAX_TOTAL_REQUESTS: u32 = 500;
const MAX_CONCURRENCY: u16 = 64;
const MAX_MAX_TOKENS: u32 = 4096;
const MAX_TPS_BENCH_RUNS: usize = 200;
const BENCHMARK_PROMPT: &str =
    "Benchmark prompt: explain the Fibonacci sequence in one short paragraph.";

static TPS_BENCH_RUNS: LazyLock<RwLock<HashMap<Uuid, TpsBenchmarkRun>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// POST /api/benchmarks/tps request body.
#[derive(Debug, Clone, Deserialize)]
pub struct StartTpsBenchmarkRequest {
    /// Target model ID.
    pub model: String,
    /// API kind to benchmark.
    #[serde(default)]
    pub api_kind: Option<TpsApiKind>,
    /// Total number of requests to execute.
    #[serde(default)]
    pub total_requests: Option<u32>,
    /// Concurrent workers.
    #[serde(default)]
    pub concurrency: Option<u16>,
    /// max_tokens / max_output_tokens
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// Sampling temperature.
    #[serde(default)]
    pub temperature: Option<f32>,
}

/// Normalized benchmark configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpsBenchmarkRequest {
    /// Target model ID.
    pub model: String,
    /// API kind to benchmark.
    pub api_kind: TpsApiKind,
    /// Total number of requests to execute.
    pub total_requests: u32,
    /// Concurrent workers.
    pub concurrency: u16,
    /// max_tokens / max_output_tokens
    pub max_tokens: u32,
    /// Sampling temperature.
    pub temperature: f32,
}

impl TryFrom<StartTpsBenchmarkRequest> for TpsBenchmarkRequest {
    type Error = AppError;

    fn try_from(value: StartTpsBenchmarkRequest) -> Result<Self, Self::Error> {
        let model = value.model.trim().to_string();
        if model.is_empty() {
            return Err(AppError::from(LbError::Common(CommonError::Validation(
                "model is required".to_string(),
            ))));
        }

        let total_requests = value.total_requests.unwrap_or(DEFAULT_TOTAL_REQUESTS);
        if total_requests == 0 || total_requests > MAX_TOTAL_REQUESTS {
            return Err(AppError::from(LbError::Common(CommonError::Validation(
                format!(
                    "total_requests must be between 1 and {}",
                    MAX_TOTAL_REQUESTS
                ),
            ))));
        }

        let concurrency = value.concurrency.unwrap_or(DEFAULT_CONCURRENCY);
        if concurrency == 0 || concurrency > MAX_CONCURRENCY {
            return Err(AppError::from(LbError::Common(CommonError::Validation(
                format!("concurrency must be between 1 and {}", MAX_CONCURRENCY),
            ))));
        }

        let max_tokens = value.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
        if max_tokens == 0 || max_tokens > MAX_MAX_TOKENS {
            return Err(AppError::from(LbError::Common(CommonError::Validation(
                format!("max_tokens must be between 1 and {}", MAX_MAX_TOKENS),
            ))));
        }

        let temperature = value.temperature.unwrap_or(DEFAULT_TEMPERATURE);
        if !(0.0..=2.0).contains(&temperature) {
            return Err(AppError::from(LbError::Common(CommonError::Validation(
                "temperature must be between 0.0 and 2.0".to_string(),
            ))));
        }

        Ok(Self {
            model,
            api_kind: value.api_kind.unwrap_or(TpsApiKind::ChatCompletions),
            total_requests,
            concurrency,
            max_tokens,
            temperature,
        })
    }
}

/// Accepted response for benchmark start.
#[derive(Debug, Clone, Serialize)]
pub struct TpsBenchmarkAccepted {
    /// Benchmark run ID.
    pub run_id: Uuid,
    /// Initial run status.
    pub status: TpsBenchmarkStatus,
}

/// Benchmark run status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TpsBenchmarkStatus {
    /// Run is still executing.
    Running,
    /// Run completed successfully.
    Completed,
    /// Run failed with error.
    Failed,
}

/// Benchmark per-endpoint summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpsBenchmarkEndpointSummary {
    /// Endpoint ID.
    pub endpoint_id: Uuid,
    /// Endpoint name.
    pub endpoint_name: String,
    /// Requests sent to this endpoint.
    pub requests: u64,
    /// Successful upstream responses.
    pub successful_requests: u64,
    /// Requests with measurable TPS.
    pub measured_requests: u64,
    /// successful_requests / requests.
    pub success_rate: f64,
    /// Mean TPS for measured requests.
    pub mean_tps: Option<f64>,
    /// p50 TPS for measured requests.
    pub p50_tps: Option<f64>,
    /// p95 TPS for measured requests.
    pub p95_tps: Option<f64>,
}

/// Comparable benchmark result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpsBenchmarkResult {
    /// API kind used during this run.
    pub api_kind: TpsApiKind,
    /// Result source (`benchmark`).
    pub source: TpsSource,
    /// Requested total request count.
    pub total_requests: u64,
    /// Successful upstream responses.
    pub successful_requests: u64,
    /// Requests with measurable TPS.
    pub measured_requests: u64,
    /// successful_requests / total_requests.
    pub success_rate: f64,
    /// Mean TPS.
    pub mean_tps: Option<f64>,
    /// p50 TPS.
    pub p50_tps: Option<f64>,
    /// p95 TPS.
    pub p95_tps: Option<f64>,
    /// Per-endpoint breakdown.
    pub per_endpoint: Vec<TpsBenchmarkEndpointSummary>,
}

/// Benchmark run record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpsBenchmarkRun {
    /// Run ID.
    pub run_id: Uuid,
    /// Current status.
    pub status: TpsBenchmarkStatus,
    /// Requested time.
    pub requested_at: DateTime<Utc>,
    /// Completed time.
    pub completed_at: Option<DateTime<Utc>>,
    /// Normalized request settings.
    pub request: TpsBenchmarkRequest,
    /// Result when completed.
    pub result: Option<TpsBenchmarkResult>,
    /// Error text when failed.
    pub error: Option<String>,
}

impl TpsBenchmarkRun {
    fn new(run_id: Uuid, request: TpsBenchmarkRequest) -> Self {
        Self {
            run_id,
            status: TpsBenchmarkStatus::Running,
            requested_at: Utc::now(),
            completed_at: None,
            request,
            result: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
struct BenchmarkSample {
    endpoint_id: Uuid,
    endpoint_name: String,
    success: bool,
    tps: Option<f64>,
}

#[derive(Debug, Default)]
struct EndpointSampleAccumulator {
    endpoint_name: String,
    requests: u64,
    successful_requests: u64,
    tps_values: Vec<f64>,
}

/// POST /api/benchmarks/tps
pub async fn start_tps_benchmark(
    State(state): State<AppState>,
    Json(payload): Json<StartTpsBenchmarkRequest>,
) -> Result<(StatusCode, Json<TpsBenchmarkAccepted>), AppError> {
    let request = TpsBenchmarkRequest::try_from(payload)?;
    let run_id = Uuid::new_v4();

    {
        let mut runs = TPS_BENCH_RUNS.write().await;
        runs.insert(run_id, TpsBenchmarkRun::new(run_id, request.clone()));
        prune_tps_benchmark_runs(&mut runs);
    }

    tokio::spawn(async move {
        finalize_tps_benchmark_run(state, run_id, request).await;
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(TpsBenchmarkAccepted {
            run_id,
            status: TpsBenchmarkStatus::Running,
        }),
    ))
}

/// GET /api/benchmarks/tps/{run_id}
pub async fn get_tps_benchmark(
    Path(run_id): Path<Uuid>,
) -> Result<Json<TpsBenchmarkRun>, AppError> {
    let runs = TPS_BENCH_RUNS.read().await;
    let run = runs
        .get(&run_id)
        .cloned()
        .ok_or_else(|| AppError::from(LbError::NotFound(format!("benchmark run {}", run_id))))?;
    Ok(Json(run))
}

async fn finalize_tps_benchmark_run(state: AppState, run_id: Uuid, request: TpsBenchmarkRequest) {
    let result = execute_tps_benchmark(&state, &request).await;
    let completed_at = Utc::now();
    let mut runs = TPS_BENCH_RUNS.write().await;
    let Some(run) = runs.get_mut(&run_id) else {
        return;
    };
    run.completed_at = Some(completed_at);
    match result {
        Ok(result) => {
            run.status = TpsBenchmarkStatus::Completed;
            run.result = Some(result);
            run.error = None;
        }
        Err(err) => {
            run.status = TpsBenchmarkStatus::Failed;
            run.result = None;
            run.error = Some(err.external_message().to_string());
        }
    }
    prune_tps_benchmark_runs(&mut runs);
}

fn prune_tps_benchmark_runs(runs: &mut HashMap<Uuid, TpsBenchmarkRun>) {
    prune_tps_benchmark_runs_with_limit(runs, MAX_TPS_BENCH_RUNS);
}

fn prune_tps_benchmark_runs_with_limit(runs: &mut HashMap<Uuid, TpsBenchmarkRun>, max_runs: usize) {
    if runs.len() <= max_runs {
        return;
    }

    let mut overflow = runs.len() - max_runs;

    // Prefer pruning old finished runs before touching active ones.
    let mut completed_candidates: Vec<(Uuid, DateTime<Utc>)> = runs
        .iter()
        .filter_map(|(run_id, run)| {
            if run.status == TpsBenchmarkStatus::Running {
                None
            } else {
                let sort_key = run
                    .completed_at
                    .as_ref()
                    .unwrap_or(&run.requested_at)
                    .to_owned();
                Some((*run_id, sort_key))
            }
        })
        .collect();
    completed_candidates.sort_by_key(|(_, sort_key)| sort_key.to_owned());

    for (run_id, _) in completed_candidates {
        if overflow == 0 {
            break;
        }
        if runs.remove(&run_id).is_some() {
            overflow -= 1;
        }
    }

    if overflow == 0 {
        return;
    }

    // Fallback: when only active runs exist, prune the oldest running runs.
    let mut running_candidates: Vec<(Uuid, DateTime<Utc>)> = runs
        .iter()
        .filter_map(|(run_id, run)| {
            if run.status == TpsBenchmarkStatus::Running {
                Some((*run_id, run.requested_at))
            } else {
                None
            }
        })
        .collect();
    running_candidates.sort_by_key(|(_, requested_at)| *requested_at);

    for (run_id, _) in running_candidates.into_iter().take(overflow) {
        runs.remove(&run_id);
    }
}

async fn execute_tps_benchmark(
    state: &AppState,
    request: &TpsBenchmarkRequest,
) -> Result<TpsBenchmarkResult, LbError> {
    let total_requests = request.total_requests;
    let concurrency = request.concurrency as usize;
    let model = request.model.clone();
    let api_kind = request.api_kind;
    let max_tokens = request.max_tokens;
    let temperature = request.temperature;

    let mut final_samples: Vec<BenchmarkSample> = Vec::with_capacity(total_requests as usize);
    let samples = stream::iter(0..total_requests)
        .map(|_| {
            let state = state.clone();
            let model = model.clone();
            async move {
                run_single_benchmark_request(&state, &model, api_kind, max_tokens, temperature)
                    .await
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<Result<BenchmarkSample, LbError>>>()
        .await;

    for sample in samples {
        final_samples.push(sample?);
    }

    Ok(build_benchmark_result(
        api_kind,
        total_requests as u64,
        final_samples,
    ))
}

async fn run_single_benchmark_request(
    state: &AppState,
    model: &str,
    api_kind: TpsApiKind,
    max_tokens: u32,
    temperature: f32,
) -> Result<BenchmarkSample, LbError> {
    let endpoint = state
        .load_manager
        .select_endpoint_round_robin_ready_for_model(model)
        .await?;

    let (path, payload) = build_benchmark_payload(model, api_kind, max_tokens, temperature);
    let url = format!("{}{}", endpoint.base_url.trim_end_matches('/'), path);
    let mut req =
        state
            .http_client
            .post(url)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(
                endpoint.inference_timeout_secs as u64,
            ));
    if let Some(api_key) = &endpoint.api_key {
        req = req.bearer_auth(api_key);
    }

    let started = Instant::now();
    let response = match req.send().await {
        Ok(response) => response,
        Err(_) => {
            return Ok(BenchmarkSample {
                endpoint_id: endpoint.id,
                endpoint_name: endpoint.name,
                success: false,
                tps: None,
            })
        }
    };
    let duration = started.elapsed();
    if !response.status().is_success() {
        return Ok(BenchmarkSample {
            endpoint_id: endpoint.id,
            endpoint_name: endpoint.name,
            success: false,
            tps: None,
        });
    }

    let body = match response.json::<Value>().await {
        Ok(body) => body,
        Err(_) => {
            return Ok(BenchmarkSample {
                endpoint_id: endpoint.id,
                endpoint_name: endpoint.name,
                success: false,
                tps: None,
            })
        }
    };

    let output_tokens = extract_usage_from_response(&body)
        .and_then(|u| u.output_tokens)
        .unwrap_or(0) as u64;
    let tps = if output_tokens > 0 {
        Some(output_tokens as f64 / (duration.as_secs_f64().max(0.001)))
    } else {
        None
    };

    Ok(BenchmarkSample {
        endpoint_id: endpoint.id,
        endpoint_name: endpoint.name,
        success: true,
        tps,
    })
}

fn build_benchmark_payload(
    model: &str,
    api_kind: TpsApiKind,
    max_tokens: u32,
    temperature: f32,
) -> (&'static str, Value) {
    match api_kind {
        TpsApiKind::ChatCompletions => (
            "/v1/chat/completions",
            json!({
                "model": model,
                "messages": [{"role": "user", "content": BENCHMARK_PROMPT}],
                "stream": false,
                "max_tokens": max_tokens,
                "temperature": temperature,
            }),
        ),
        TpsApiKind::Completions => (
            "/v1/completions",
            json!({
                "model": model,
                "prompt": BENCHMARK_PROMPT,
                "stream": false,
                "max_tokens": max_tokens,
                "temperature": temperature,
            }),
        ),
        TpsApiKind::Responses => (
            "/v1/responses",
            json!({
                "model": model,
                "input": BENCHMARK_PROMPT,
                "stream": false,
                "max_output_tokens": max_tokens,
                "temperature": temperature,
            }),
        ),
    }
}

fn build_benchmark_result(
    api_kind: TpsApiKind,
    total_requests: u64,
    samples: Vec<BenchmarkSample>,
) -> TpsBenchmarkResult {
    let successful_requests = samples.iter().filter(|s| s.success).count() as u64;
    let tps_values: Vec<f64> = samples.iter().filter_map(|s| s.tps).collect();
    let measured_requests = tps_values.len() as u64;

    let mut endpoint_map: HashMap<Uuid, EndpointSampleAccumulator> = HashMap::new();
    for sample in samples {
        let entry = endpoint_map.entry(sample.endpoint_id).or_default();
        entry.endpoint_name = sample.endpoint_name;
        entry.requests += 1;
        if sample.success {
            entry.successful_requests += 1;
        }
        if let Some(tps) = sample.tps {
            entry.tps_values.push(tps);
        }
    }

    let mut per_endpoint: Vec<TpsBenchmarkEndpointSummary> = endpoint_map
        .into_iter()
        .map(|(endpoint_id, acc)| {
            let requests = acc.requests;
            let success_rate = if requests > 0 {
                acc.successful_requests as f64 / requests as f64
            } else {
                0.0
            };
            TpsBenchmarkEndpointSummary {
                endpoint_id,
                endpoint_name: acc.endpoint_name,
                requests,
                successful_requests: acc.successful_requests,
                measured_requests: acc.tps_values.len() as u64,
                success_rate,
                mean_tps: mean(&acc.tps_values),
                p50_tps: percentile(&acc.tps_values, 0.50),
                p95_tps: percentile(&acc.tps_values, 0.95),
            }
        })
        .collect();
    per_endpoint.sort_by(|a, b| a.endpoint_name.cmp(&b.endpoint_name));

    let success_rate = if total_requests > 0 {
        successful_requests as f64 / total_requests as f64
    } else {
        0.0
    };

    TpsBenchmarkResult {
        api_kind,
        source: TpsSource::Benchmark,
        total_requests,
        successful_requests,
        measured_requests,
        success_rate,
        mean_tps: mean(&tps_values),
        p50_tps: percentile(&tps_values, 0.50),
        p95_tps: percentile(&tps_values, 0.95),
        per_endpoint,
    }
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

fn percentile(values: &[f64], percentile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let index = ((sorted.len() - 1) as f64 * percentile)
        .round()
        .clamp(0.0, (sorted.len() - 1) as f64) as usize;
    sorted.get(index).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn sample_request() -> TpsBenchmarkRequest {
        TpsBenchmarkRequest {
            model: "test-model".to_string(),
            api_kind: TpsApiKind::ChatCompletions,
            total_requests: 10,
            concurrency: 2,
            max_tokens: 64,
            temperature: 0.2,
        }
    }

    fn build_run(
        requested_at: DateTime<Utc>,
        status: TpsBenchmarkStatus,
        completed_at: Option<DateTime<Utc>>,
    ) -> (Uuid, TpsBenchmarkRun) {
        let run_id = Uuid::new_v4();
        let mut run = TpsBenchmarkRun::new(run_id, sample_request());
        run.requested_at = requested_at;
        run.status = status;
        run.completed_at = completed_at;
        (run_id, run)
    }

    #[test]
    fn prune_prefers_completed_runs_before_running_runs() {
        let now = Utc::now();
        let mut runs = HashMap::new();

        let (completed_id, completed_run) = build_run(
            now - Duration::minutes(3),
            TpsBenchmarkStatus::Completed,
            Some(now - Duration::minutes(2)),
        );
        let (running_old_id, running_old_run) = build_run(
            now - Duration::minutes(2),
            TpsBenchmarkStatus::Running,
            None,
        );
        let (running_new_id, running_new_run) = build_run(
            now - Duration::minutes(1),
            TpsBenchmarkStatus::Running,
            None,
        );

        runs.insert(completed_id, completed_run);
        runs.insert(running_old_id, running_old_run);
        runs.insert(running_new_id, running_new_run);

        prune_tps_benchmark_runs_with_limit(&mut runs, 2);

        assert_eq!(runs.len(), 2);
        assert!(!runs.contains_key(&completed_id));
        assert!(runs.contains_key(&running_old_id));
        assert!(runs.contains_key(&running_new_id));
    }

    #[test]
    fn prune_removes_oldest_running_when_all_runs_are_running() {
        let now = Utc::now();
        let mut runs = HashMap::new();

        let (oldest_id, oldest_run) = build_run(
            now - Duration::minutes(3),
            TpsBenchmarkStatus::Running,
            None,
        );
        let (middle_id, middle_run) = build_run(
            now - Duration::minutes(2),
            TpsBenchmarkStatus::Running,
            None,
        );
        let (newest_id, newest_run) = build_run(
            now - Duration::minutes(1),
            TpsBenchmarkStatus::Running,
            None,
        );

        runs.insert(oldest_id, oldest_run);
        runs.insert(middle_id, middle_run);
        runs.insert(newest_id, newest_run);

        prune_tps_benchmark_runs_with_limit(&mut runs, 2);

        assert_eq!(runs.len(), 2);
        assert!(!runs.contains_key(&oldest_id));
        assert!(runs.contains_key(&middle_id));
        assert!(runs.contains_key(&newest_id));
    }
}
