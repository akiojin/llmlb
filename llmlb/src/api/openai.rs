//! OpenAI互換APIエンドポイント (/v1/*)
//!
//! このモジュールはEndpointRegistry/Endpoint型を使用しています。

/// 未指定/仮想IPアドレス（クラウドプロバイダ等、実IPを持たない場合に使用）
const UNSPECIFIED_IP: std::net::IpAddr = std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED);

use crate::common::{
    error::{CommonError, LbError},
    protocol::{RecordStatus, RequestResponseRecord, RequestType, TpsApiKind},
};
use crate::types::model::{ModelCapabilities, ModelCapability};
use axum::body::Body;
use axum::{
    extract::{ConnectInfo, Path, State},
    http::{header::CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use reqwest;
use serde_json::{json, Value};
use std::{collections::HashMap, net::IpAddr, net::SocketAddr, time::Instant};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::auth::middleware::ApiKeyAuthContext;
use crate::common::ip::{normalize_ip, normalize_socket_ip};

use crate::{
    api::{
        error::AppError,
        model_name::{parse_quantized_model_name, ParsedModelName},
        models::{list_registered_models, load_registered_model, LifecycleStatus},
        proxy::{
            forward_streaming_response, forward_streaming_response_with_tps_tracking,
            record_endpoint_request_stats, save_request_record, select_available_endpoint,
            select_available_endpoint_with_queue_for_model, QueueSelection,
        },
    },
    balancer::RequestOutcome,
    cloud_metrics,
    token::extract_usage_from_response,
    AppState,
};

fn map_reqwest_error(err: reqwest::Error) -> AppError {
    AppError::from(LbError::Http(err.to_string()))
}

fn auth_error(msg: &str) -> AppError {
    AppError::from(LbError::Authentication(msg.to_string()))
}

fn get_required_key(provider: &str, env_key: &str, err_msg: &str) -> Result<String, AppError> {
    match std::env::var(env_key) {
        Ok(v) => {
            info!(provider = provider, key = env_key, "cloud api key present");
            Ok(v)
        }
        Err(_) => {
            warn!(provider = provider, key = env_key, "cloud api key missing");
            Err(auth_error(err_msg))
        }
    }
}

/// SPEC-f8e3a1b7: 推論リクエスト成功時にエンドポイントのレイテンシを更新（Fire-and-forget）
fn update_inference_latency(
    registry: &crate::registry::endpoints::EndpointRegistry,
    endpoint_id: Uuid,
    duration: std::time::Duration,
) {
    let registry = registry.clone();
    let latency_ms = duration.as_millis() as f64;
    tokio::spawn(async move {
        if let Err(e) = registry
            .update_inference_latency(endpoint_id, latency_ms)
            .await
        {
            tracing::debug!(
                endpoint_id = %endpoint_id,
                latency_ms = latency_ms,
                error = %e,
                "Failed to update inference latency"
            );
        }
    });
}

fn sanitize_openai_payload_for_history(payload: &Value) -> Value {
    fn redact_data_url(value: &Value) -> Value {
        match value {
            Value::String(s) => {
                if s.starts_with("data:") && s.contains(";base64,") {
                    Value::String(format!("[redacted data-url len={}]", s.len()))
                } else {
                    Value::String(s.clone())
                }
            }
            Value::Array(items) => Value::Array(items.iter().map(redact_data_url).collect()),
            Value::Object(map) => {
                let mut out = serde_json::Map::with_capacity(map.len());
                for (k, v) in map {
                    if k == "input_audio" {
                        if let Some(obj) = v.as_object() {
                            let mut cloned = obj.clone();
                            if let Some(data) = obj.get("data").and_then(|d| d.as_str()) {
                                cloned.insert(
                                    "data".to_string(),
                                    Value::String(format!("[redacted base64 len={}]", data.len())),
                                );
                            }
                            out.insert(k.clone(), Value::Object(cloned));
                            continue;
                        }
                    }

                    if k == "image_url" {
                        if let Some(obj) = v.as_object() {
                            let mut cloned = obj.clone();
                            if let Some(url) = obj.get("url").and_then(|d| d.as_str()) {
                                if url.starts_with("data:") && url.contains(";base64,") {
                                    cloned.insert(
                                        "url".to_string(),
                                        Value::String(format!(
                                            "[redacted data-url len={}]",
                                            url.len()
                                        )),
                                    );
                                }
                            }
                            out.insert(k.clone(), Value::Object(cloned));
                            continue;
                        }
                    }

                    out.insert(k.clone(), redact_data_url(v));
                }
                Value::Object(out)
            }
            _ => value.clone(),
        }
    }

    redact_data_url(payload)
}

fn add_queue_headers(response: &mut Response, wait_ms: u128) {
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-queue-status"),
        HeaderValue::from_static("queued"),
    );
    let wait_value = wait_ms.to_string();
    if let Ok(value) = HeaderValue::from_str(&wait_value) {
        headers.insert(HeaderName::from_static("x-queue-wait-ms"), value);
    }
}

fn queue_error_response(
    status: StatusCode,
    message: &str,
    error_type: &str,
    retry_after: Option<u64>,
) -> Response {
    let mut response = (
        status,
        Json(json!({
            "error": {
                "message": message,
                "type": error_type,
                "code": status.as_u16(),
            }
        })),
    )
        .into_response();

    if let Some(value) = retry_after {
        if let Ok(header_value) = HeaderValue::from_str(&value.to_string()) {
            response
                .headers_mut()
                .insert(HeaderName::from_static("retry-after"), header_value);
        }
    }

    response
}

fn openai_error_response(message: impl Into<String>, status: StatusCode) -> Response {
    let payload = json!({
        "error": {
            "message": message.into(),
            "type": "invalid_request_error",
            "code": status.as_u16(),
        }
    });

    (status, Json(payload)).into_response()
}

fn model_unavailable_response(message: impl Into<String>, code: &str) -> Response {
    let payload = json!({
        "error": {
            "message": message.into(),
            "type": "service_unavailable",
            "code": code,
        }
    });

    (StatusCode::SERVICE_UNAVAILABLE, Json(payload)).into_response()
}

/// クライアントIPとAPIキーIDを抽出するヘルパー
fn extract_client_info(
    addr: &SocketAddr,
    headers: &HeaderMap,
    auth_ctx: &Option<axum::Extension<ApiKeyAuthContext>>,
) -> (Option<IpAddr>, Option<Uuid>) {
    let client_ip =
        Some(extract_client_ip_from_headers(headers).unwrap_or_else(|| normalize_socket_ip(addr)));
    let api_key_id = auth_ctx.as_ref().map(|ext| ext.0.id);
    (client_ip, api_key_id)
}

fn extract_client_ip_from_headers(headers: &HeaderMap) -> Option<IpAddr> {
    extract_x_forwarded_for(headers).or_else(|| extract_forwarded_for(headers))
}

fn extract_x_forwarded_for(headers: &HeaderMap) -> Option<IpAddr> {
    let value = headers.get("x-forwarded-for")?.to_str().ok()?;
    value
        .split(',')
        .map(str::trim)
        .find_map(parse_client_ip_from_forwarded_value)
}

fn extract_forwarded_for(headers: &HeaderMap) -> Option<IpAddr> {
    let value = headers.get("forwarded")?.to_str().ok()?;
    value.split(',').find_map(|entry| {
        entry
            .split(';')
            .filter_map(|pair| pair.split_once('='))
            .find_map(|(key, value)| {
                if key.trim().eq_ignore_ascii_case("for") {
                    parse_client_ip_from_forwarded_value(value.trim())
                } else {
                    None
                }
            })
    })
}

fn parse_client_ip_from_forwarded_value(value: &str) -> Option<IpAddr> {
    let trimmed = value.trim().trim_matches('"');
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("unknown") || trimmed.starts_with('_') {
        return None;
    }

    let host = if let Some(stripped) = trimmed.strip_prefix('[') {
        stripped.split(']').next().unwrap_or_default().trim()
    } else {
        trimmed
    };

    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(normalize_ip(ip));
    }

    if let Some((ip_candidate, _port)) = host.rsplit_once(':') {
        if !ip_candidate.contains(':') {
            if let Ok(ip) = ip_candidate.parse::<IpAddr>() {
                return Some(normalize_ip(ip));
            }
        }
    }

    None
}

/// POST /v1/chat/completions - OpenAI互換チャットAPI
#[allow(deprecated)] // NodeRegistry migration in progress
pub async fn chat_completions(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(state): State<AppState>,
    auth_ctx: Option<axum::Extension<ApiKeyAuthContext>>,
    Json(payload): Json<Value>,
) -> Result<Response, AppError> {
    let (client_ip, api_key_id) = extract_client_info(&addr, &headers, &auth_ctx);
    let model = extract_model(&payload)?;
    let parsed = if parse_cloud_model(&model).is_some() {
        ParsedModelName {
            raw: model.clone(),
            base: model.clone(),
            quantization: None,
        }
    } else {
        parse_quantized_model_name(&model).map_err(AppError::from)?
    };

    // モデルの TextGeneration capability を検証
    let models = list_registered_models(&state.db_pool).await?;
    if let Some(model_info) = models.iter().find(|m| m.name == model) {
        if !model_info.has_capability(ModelCapability::TextGeneration) {
            return Err(AppError::from(LbError::Common(CommonError::Validation(
                format!("Model '{}' does not support text generation", parsed.raw),
            ))));
        }
    }
    // 登録されていないモデルはエンドポイント側で処理（クラウドモデル等）

    if let Some(response) = reject_image_payload(&payload) {
        return Ok(response);
    }

    let stream = extract_stream(&payload);
    proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        parsed.raw,
        stream,
        RequestType::Chat,
        client_ip,
        api_key_id,
    )
    .await
}

/// POST /v1/completions - OpenAI互換テキスト補完API
pub async fn completions(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(state): State<AppState>,
    auth_ctx: Option<axum::Extension<ApiKeyAuthContext>>,
    Json(payload): Json<Value>,
) -> Result<Response, AppError> {
    let (client_ip, api_key_id) = extract_client_info(&addr, &headers, &auth_ctx);
    let model = extract_model(&payload)?;
    if parse_cloud_model(&model).is_none() {
        parse_quantized_model_name(&model).map_err(AppError::from)?;
    }
    let stream = extract_stream(&payload);
    proxy_openai_post(
        &state,
        payload,
        "/v1/completions",
        model,
        stream,
        RequestType::Generate,
        client_ip,
        api_key_id,
    )
    .await
}

/// POST /v1/embeddings - OpenAI互換Embeddings API
pub async fn embeddings(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(state): State<AppState>,
    auth_ctx: Option<axum::Extension<ApiKeyAuthContext>>,
    Json(payload): Json<Value>,
) -> Result<Response, AppError> {
    let (client_ip, api_key_id) = extract_client_info(&addr, &headers, &auth_ctx);
    let model = extract_model_with_default(&payload, crate::config::get_default_embedding_model());
    if parse_cloud_model(&model).is_none() {
        parse_quantized_model_name(&model).map_err(AppError::from)?;
    }
    proxy_openai_post(
        &state,
        payload,
        "/v1/embeddings",
        model,
        false,
        RequestType::Embeddings,
        client_ip,
        api_key_id,
    )
    .await
}

/// GET /v1/models - モデル一覧取得（OpenAI互換 + Azure capabilities + ダッシュボード拡張）
///
/// OpenAI API 互換形式に Azure OpenAI 形式の capabilities と
/// ダッシュボード用の拡張フィールド（lifecycle_status, download_progress, ready）を追加。
/// 登録済みの全モデルを返す（ダウンロード中・待機中含む）。
pub async fn list_models(State(state): State<AppState>) -> Result<Response, AppError> {
    use crate::types::endpoint::SupportedAPI;
    use std::collections::HashSet;

    // Load registered models from the database.
    let mut registered_map: std::collections::HashMap<String, crate::registry::models::ModelInfo> =
        HashMap::new();
    for model in list_registered_models(&state.db_pool).await? {
        registered_map.insert(model.name.clone(), model);
    }

    // SPEC-0f1de549: エンドポイントのモデルとsupported_apisを取得
    let mut endpoint_model_apis: HashMap<String, HashSet<SupportedAPI>> = HashMap::new();
    let mut endpoint_model_max_tokens: HashMap<String, Option<u32>> = HashMap::new();
    let mut endpoint_model_ids: HashMap<String, HashSet<String>> = HashMap::new();
    {
        let registry = &state.endpoint_registry;
        let online_endpoints = registry.list_online().await;
        for ep in online_endpoints {
            if let Ok(models) = registry.list_models(ep.id).await {
                for model in models {
                    endpoint_model_ids
                        .entry(model.model_id.clone())
                        .or_default()
                        .insert(ep.id.to_string());
                    let apis = endpoint_model_apis
                        .entry(model.model_id.clone())
                        .or_default();
                    for api in model.supported_apis {
                        apis.insert(api);
                    }
                    // Responses APIは全エンドポイント対応前提（判定/フラグは廃止）
                    apis.insert(SupportedAPI::Responses);

                    // max_tokens を集約（複数エンドポイントにある場合は最大値を採用）
                    let entry = endpoint_model_max_tokens
                        .entry(model.model_id.clone())
                        .or_insert(None);
                    if let Some(mt) = model.max_tokens {
                        *entry = Some(entry.map_or(mt, |existing| existing.max(mt)));
                    }
                }
            }
        }
    }

    // オンラインエンドポイントの実行可能モデル一覧を構築
    let mut available_models: Vec<String> = endpoint_model_apis.keys().cloned().collect();
    available_models.sort();
    let available_set: std::collections::HashSet<String> =
        available_models.iter().cloned().collect();

    // 追跡用：モデルID一覧
    let mut seen_models: HashSet<String> = HashSet::new();

    // OpenAI互換レスポンス形式 + Azure capabilities + ダッシュボード拡張
    let mut data: Vec<Value> = Vec::new();

    // ノードのモデルを追加
    for model_id in &available_models {
        seen_models.insert(model_id.clone());
        let ready = available_set.contains(model_id);

        // supported_apisを取得（デフォルトはchat_completions）
        let supported_apis: Vec<String> = endpoint_model_apis
            .get(model_id)
            .map(|apis| apis.iter().map(|a| a.as_str().to_string()).collect())
            .unwrap_or_else(|| vec!["chat_completions".to_string()]);
        let endpoint_ids: Vec<String> = endpoint_model_ids
            .get(model_id)
            .map(|ids| {
                let mut ids: Vec<String> = ids.iter().cloned().collect();
                ids.sort();
                ids
            })
            .unwrap_or_default();

        if let Some(m) = registered_map.get(model_id) {
            let caps: ModelCapabilities = m.get_capabilities().into();
            let obj = json!({
                "id": m.name,
                "object": "model",
                "created": 0,
                "owned_by": "load balancer",
                "capabilities": caps,
                "lifecycle_status": LifecycleStatus::Registered,
                "download_progress": null,
                "ready": ready,
                "repo": m.repo,
                "filename": m.filename,
                "size_bytes": m.size,
                "required_memory_bytes": m.required_memory,
                "source": m.source,
                "tags": m.tags,
                "description": m.description,
                "chat_template": m.chat_template,
                "supported_apis": supported_apis,
                "max_tokens": endpoint_model_max_tokens.get(model_id).copied().flatten(),
                "endpoint_ids": endpoint_ids,
            });
            data.push(obj);
        } else {
            let obj = json!({
                "id": model_id,
                "object": "model",
                "created": 0,
                "owned_by": "load balancer",
                "lifecycle_status": LifecycleStatus::Registered,
                "download_progress": null,
                "ready": ready,
                "supported_apis": supported_apis,
                "max_tokens": endpoint_model_max_tokens.get(model_id).copied().flatten(),
                "endpoint_ids": endpoint_ids,
            });
            data.push(obj);
        }
    }

    // SPEC-0f1de549: エンドポイント専用モデルを追加（ノードにないモデル）
    for (model_id, apis) in &endpoint_model_apis {
        if seen_models.contains(model_id) {
            continue;
        }
        seen_models.insert(model_id.clone());

        let supported_apis: Vec<String> = apis.iter().map(|a| a.as_str().to_string()).collect();
        let endpoint_ids: Vec<String> = endpoint_model_ids
            .get(model_id)
            .map(|ids| {
                let mut ids: Vec<String> = ids.iter().cloned().collect();
                ids.sort();
                ids
            })
            .unwrap_or_default();
        let obj = json!({
            "id": model_id,
            "object": "model",
            "created": 0,
            "owned_by": "endpoint",
            "lifecycle_status": LifecycleStatus::Registered,
            "download_progress": null,
            "ready": true,
            "supported_apis": supported_apis,
            "max_tokens": endpoint_model_max_tokens.get(model_id).copied().flatten(),
            "endpoint_ids": endpoint_ids,
        });
        data.push(obj);
    }

    // NOTE: SPEC-6cd7f960 FR-6により、登録済みだがオンラインエンドポイントにないモデルは
    // /v1/models に含めない（利用可能なモデルのみを返す）

    // クラウドプロバイダーのモデル一覧を追加（SPEC-996e37bf）

    let cloud_models = super::cloud_models::get_cached_models(&state.http_client).await;
    for cm in cloud_models {
        let obj = json!({
            "id": cm.id,
            "object": cm.object,
            "created": cm.created,
            "owned_by": cm.owned_by,
            // クラウドモデルはリモートで常に利用可能
            "lifecycle_status": LifecycleStatus::Registered,
            "download_progress": null,
            "ready": true,
            "supported_apis": vec!["chat_completions"],
            "max_tokens": null,
            "endpoint_ids": Vec::<String>::new(),
        });
        data.push(obj);
    }

    let body = json!({
        "object": "list",
        "data": data,
    });

    Ok((StatusCode::OK, Json(body)).into_response())
}

// NOTE: list_models_extended() は廃止されました。
// /v1/models に Azure OpenAI 形式の capabilities とダッシュボード拡張が統合されています。

/// GET /v1/models/:id - モデル詳細取得（Azure capabilities 形式）
///
/// SPEC-0f1de549: Endpoints APIで登録されたモデルも検索対象に含める
#[allow(deprecated)] // NodeRegistry migration in progress
pub async fn get_model(
    State(state): State<AppState>,
    Path(model_id): Path<String>,
) -> Result<Response, AppError> {
    use crate::types::endpoint::SupportedAPI;
    use std::collections::HashSet;

    let mut registered_map: HashMap<String, crate::registry::models::ModelInfo> = HashMap::new();
    for model in list_registered_models(&state.db_pool).await? {
        registered_map.insert(model.name.clone(), model);
    }

    // SPEC-0f1de549: エンドポイントのモデルとsupported_apisを取得
    let mut endpoint_model_apis: HashMap<String, HashSet<SupportedAPI>> = HashMap::new();
    {
        let registry = &state.endpoint_registry;
        let online_endpoints = registry.list_online().await;
        for ep in online_endpoints {
            if let Ok(models) = registry.list_models(ep.id).await {
                for model in models {
                    let apis = endpoint_model_apis
                        .entry(model.model_id.clone())
                        .or_default();
                    for api in model.supported_apis {
                        apis.insert(api);
                    }
                    apis.insert(SupportedAPI::Responses);
                }
            }
        }
    }

    let model = registered_map.remove(&model_id);
    let is_endpoint_model = endpoint_model_apis.contains_key(&model_id);

    if model.is_none() && !is_endpoint_model {
        // 404 を OpenAI 換算で返す
        let body = json!({
            "error": {
                "message": "The model does not exist",
                "type": "invalid_request_error",
                "param": "model",
                "code": "model_not_found"
            }
        });
        return Ok((StatusCode::NOT_FOUND, Json(body)).into_response());
    }

    // supported_apisを取得（デフォルトはchat_completions）
    let supported_apis: Vec<String> = endpoint_model_apis
        .get(&model_id)
        .map(|apis| apis.iter().map(|a| a.as_str().to_string()).collect())
        .unwrap_or_else(|| vec!["chat_completions".to_string()]);

    if let Some(model) = model {
        // Azure OpenAI 形式の capabilities (boolean object)
        let caps: ModelCapabilities = model.get_capabilities().into();
        let ready = is_endpoint_model;
        let lifecycle_status = if ready {
            LifecycleStatus::Registered
        } else {
            LifecycleStatus::Pending
        };

        let body = json!({
            "id": model_id,
            "object": "model",
            "created": 0,
            "owned_by": "load balancer",
            "capabilities": caps,
            // ダッシュボード用拡張フィールド
            "lifecycle_status": lifecycle_status,
            "ready": ready,
            // 追加メタデータ（ダッシュボード向け）
            "repo": model.repo,
            "filename": model.filename,
            "size_bytes": model.size,
            "required_memory_bytes": model.required_memory,
            "source": model.source,
            "tags": model.tags,
            "description": model.description,
            "chat_template": model.chat_template,
            "supported_apis": supported_apis,
        });

        return Ok((StatusCode::OK, Json(body)).into_response());
    }

    // エンドポイント専用モデルまたはノードのモデル（メタデータなし）
    let owned_by = if is_endpoint_model {
        "endpoint"
    } else {
        "load balancer"
    };

    let body = json!({
        "id": model_id,
        "object": "model",
        "created": 0,
        "owned_by": owned_by,
        "lifecycle_status": LifecycleStatus::Registered,
        "ready": true,
        "supported_apis": supported_apis,
    });

    Ok((StatusCode::OK, Json(body)).into_response())
}

fn extract_model(payload: &Value) -> Result<String, AppError> {
    payload
        .get("model")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| validation_error("`model` field is required for OpenAI-compatible requests"))
}

/// モデル名を抽出し、未指定または空の場合はデフォルト値を使用
fn extract_model_with_default(payload: &Value, default: String) -> String {
    payload
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or(default)
}

fn extract_stream(payload: &Value) -> bool {
    payload
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn reject_image_payload(payload: &Value) -> Option<Response> {
    let messages = payload.get("messages").and_then(|v| v.as_array())?;

    for message in messages {
        let Some(parts) = message.get("content").and_then(|v| v.as_array()) else {
            continue;
        };
        for part in parts {
            if part.get("type").and_then(|v| v.as_str()) == Some("image_url") {
                return Some(openai_error_response(
                    "Image inputs are not supported",
                    StatusCode::BAD_REQUEST,
                ));
            }
        }
    }

    None
}

fn parse_cloud_model(model: &str) -> Option<(String, String)> {
    // Accept prefixes like "openai:foo", "google:bar", "anthropic:baz"
    let prefixes = ["openai:", "google:", "anthropic:", "ahtnorpic:"];
    for p in prefixes.iter() {
        if model.starts_with(p) {
            let rest = model.trim_start_matches(p);
            if rest.is_empty() {
                return None;
            }
            let provider = if *p == "ahtnorpic:" {
                "anthropic"
            } else {
                p.trim_end_matches(':')
            };
            return Some((provider.to_string(), rest.to_string()));
        }
    }
    None
}

/// クラウドプロバイダ用の仮想ノード情報を生成する
fn cloud_virtual_node(provider: &str) -> (Uuid, String, IpAddr) {
    // 仮想ノードIDはクラウドプロバイダごとに固定値
    let endpoint_id = match provider {
        "openai" => Uuid::parse_str("00000000-0000-0000-0000-00000000c001")
            .expect("static UUID string is valid"),
        "google" => Uuid::parse_str("00000000-0000-0000-0000-00000000c002")
            .expect("static UUID string is valid"),
        "anthropic" => Uuid::parse_str("00000000-0000-0000-0000-00000000c003")
            .expect("static UUID string is valid"),
        _ => Uuid::parse_str("00000000-0000-0000-0000-00000000c0ff")
            .expect("static UUID string is valid"),
    };
    let machine_name = format!("cloud:{provider}");
    (endpoint_id, machine_name, UNSPECIFIED_IP)
}

struct CloudProxyResult {
    response: Response,
    response_body: Option<Value>,
    status: StatusCode,
    error_message: Option<String>,
}

fn map_openai_messages_to_google_contents(messages: &[Value]) -> Vec<Value> {
    messages
        .iter()
        .filter_map(|m| {
            let role = m.get("role")?.as_str().unwrap_or("user");
            let text = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
            let mapped_role = match role {
                "assistant" => "model",
                _ => "user",
            };
            Some(json!({
                "role": mapped_role,
                "parts": [{"text": text}]
            }))
        })
        .collect()
}

fn map_openai_messages_to_anthropic(messages: &[Value]) -> (Option<String>, Vec<Value>) {
    let mut system_msgs: Vec<String> = Vec::new();
    let mut regular: Vec<Value> = Vec::new();
    for m in messages.iter() {
        let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        let text = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
        match role {
            "system" => system_msgs.push(text.to_string()),
            "assistant" => regular.push(json!({
                "role": "assistant",
                "content": [{"type":"text","text": text}]
            })),
            _ => regular.push(json!({
                "role": "user",
                "content": [{"type":"text","text": text}]
            })),
        }
    }
    let system = if system_msgs.is_empty() {
        None
    } else {
        Some(system_msgs.join("\n"))
    };
    (system, regular)
}

async fn proxy_openai_provider(
    http_client: &reqwest::Client,
    target_path: &str,
    mut payload: Value,
    stream: bool,
    model: String,
) -> Result<CloudProxyResult, AppError> {
    let req_id = Uuid::new_v4();
    let started = Instant::now();
    let api_key = get_required_key(
        "openai",
        "OPENAI_API_KEY",
        "OPENAI_API_KEY is required for openai: models",
    )?;
    let base = std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com".into());

    // strip provider prefix before forwarding
    payload["model"] = Value::String(model);

    let url = format!("{base}{target_path}");
    let res = http_client
        .post(&url)
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await
        .map_err(map_reqwest_error)?;

    if stream {
        info!(
            provider = "openai",
            model = payload.get("model").and_then(|v| v.as_str()).unwrap_or(""),
            request_id = %req_id,
            latency_ms = started.elapsed().as_millis(),
            stream = true,
            status = %res.status(),
            "cloud proxy stream (openai)"
        );
        cloud_metrics::record(
            "openai",
            res.status().as_u16(),
            started.elapsed().as_millis(),
        );
        let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let response = forward_streaming_response(res).map_err(AppError::from)?;
        return Ok(CloudProxyResult {
            response,
            response_body: None,
            status,
            error_message: if status.is_success() {
                None
            } else {
                Some(status.to_string())
            },
        });
    }

    let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let ct = res.headers().get(reqwest::header::CONTENT_TYPE).cloned();
    let bytes = res.bytes().await.map_err(map_reqwest_error)?;
    let parsed_body = serde_json::from_slice::<Value>(&bytes).ok();
    let error_message = if status.is_success() {
        None
    } else {
        Some(String::from_utf8_lossy(&bytes).trim().to_string())
    };
    let mut resp = Response::builder().status(status);
    if let Some(ct) = ct {
        if let Ok(hv) = HeaderValue::from_str(ct.to_str().unwrap_or("")) {
            resp = resp.header(CONTENT_TYPE, hv);
        }
    }
    let built = resp
        .body(Body::from(bytes))
        .expect("Response builder should not fail with valid status and bytes body");
    info!(
        provider = "openai",
        model = payload.get("model").and_then(|v| v.as_str()).unwrap_or(""),
        request_id = %req_id,
        latency_ms = started.elapsed().as_millis(),
        stream = false,
        status = %status,
        "cloud proxy complete (openai)"
    );
    cloud_metrics::record("openai", status.as_u16(), started.elapsed().as_millis());
    Ok(CloudProxyResult {
        response: built,
        response_body: parsed_body,
        status,
        error_message,
    })
}

fn map_generation_config(payload: &Value) -> Value {
    json!({
        "temperature": payload.get("temperature").and_then(|v| v.as_f64()),
        "topP": payload.get("top_p").and_then(|v| v.as_f64()),
        "maxOutputTokens": payload.get("max_tokens").and_then(|v| v.as_i64()),
    })
}

async fn proxy_google_provider(
    http_client: &reqwest::Client,
    model: String,
    payload: Value,
    stream: bool,
) -> Result<CloudProxyResult, AppError> {
    let req_id = Uuid::new_v4();
    let started = Instant::now();
    let api_key = get_required_key(
        "google",
        "GOOGLE_API_KEY",
        "GOOGLE_API_KEY is required for google: models",
    )?;
    let base = std::env::var("GOOGLE_API_BASE_URL")
        .unwrap_or_else(|_| "https://generativelanguage.googleapis.com/v1beta".into());
    let messages = payload
        .get("messages")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();
    let contents = map_openai_messages_to_google_contents(&messages);
    let mut body = json!({
        "contents": contents,
        "generationConfig": map_generation_config(&payload),
    });
    // drop nulls in generationConfig
    if let Some(gen) = body["generationConfig"].as_object_mut() {
        gen.retain(|_, v| !v.is_null());
    }

    let endpoint_suffix = if stream {
        format!("models/{model}:streamGenerateContent")
    } else {
        format!("models/{model}:generateContent")
    };
    let url = format!("{base}/{endpoint_suffix}");

    let req = http_client
        .post(&url)
        .query(&[("key", api_key)])
        .json(&body);
    let res = req.send().await.map_err(map_reqwest_error)?;

    if stream {
        info!(
            provider = "google",
            model = %model,
            request_id = %req_id,
            latency_ms = started.elapsed().as_millis(),
            stream = true,
            status = %res.status(),
            "cloud proxy stream (google)"
        );
        cloud_metrics::record(
            "google",
            res.status().as_u16(),
            started.elapsed().as_millis(),
        );
        let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let response = forward_streaming_response(res).map_err(AppError::from)?;
        return Ok(CloudProxyResult {
            response,
            response_body: None,
            status,
            error_message: if status.is_success() {
                None
            } else {
                Some(status.to_string())
            },
        });
    }

    let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let data: Value = res.json().await.map_err(map_reqwest_error)?;
    let text = data
        .get("candidates")
        .and_then(|c: &Value| c.get(0))
        .and_then(|c: &Value| c.get("content"))
        .and_then(|c: &Value| c.get("parts"))
        .and_then(|p: &Value| p.get(0))
        .and_then(|p: &Value| p.get("text"))
        .and_then(|t: &Value| t.as_str())
        .unwrap_or("");

    let resp_body = json!({
        "id": format!("google-{}", Uuid::new_v4()),
        "object": "chat.completion",
        "model": format!("google:{model}"),
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": text},
        "finish_reason": "stop"
    }],
    });

    let built = Response::builder()
        .status(status)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(Body::from(resp_body.to_string()))
        .map_err(|e| AppError::from(LbError::Http(e.to_string())))?;

    info!(
        provider = "google",
        model = %model,
        request_id = %req_id,
        latency_ms = started.elapsed().as_millis(),
        stream = false,
        status = %status,
        "cloud proxy complete (google)"
    );

    cloud_metrics::record("google", status.as_u16(), started.elapsed().as_millis());

    let error_message = if status.is_success() {
        None
    } else {
        serde_json::to_string(&data).ok()
    };

    Ok(CloudProxyResult {
        response: built,
        response_body: Some(resp_body),
        status,
        error_message,
    })
}

async fn proxy_anthropic_provider(
    http_client: &reqwest::Client,
    model: String,
    payload: Value,
    stream: bool,
) -> Result<CloudProxyResult, AppError> {
    let req_id = Uuid::new_v4();
    let started = Instant::now();
    let api_key = get_required_key(
        "anthropic",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_API_KEY is required for anthropic: models",
    )?;
    let base = std::env::var("ANTHROPIC_API_BASE_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com".into());
    let messages = payload
        .get("messages")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();
    let (system, mapped) = map_openai_messages_to_anthropic(&messages);
    let max_tokens = payload
        .get("max_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(1024);
    let mut body = json!({
        "model": model,
        "messages": mapped,
        "max_tokens": max_tokens,
        "stream": stream,
        "temperature": payload.get("temperature").and_then(|v| v.as_f64()),
        "top_p": payload.get("top_p").and_then(|v| v.as_f64()),
    });
    if let Some(s) = system {
        body["system"] = Value::String(s);
    }
    // prune nulls
    if let Some(obj) = body.as_object_mut() {
        obj.retain(|_, v| !v.is_null());
    }

    let url = format!("{base}/v1/messages");
    let req = http_client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body);
    let res = req.send().await.map_err(map_reqwest_error)?;

    if stream {
        info!(
            provider = "anthropic",
            model = %model,
            request_id = %req_id,
            latency_ms = started.elapsed().as_millis(),
            stream = true,
            status = %res.status(),
            "cloud proxy stream (anthropic)"
        );
        cloud_metrics::record(
            "anthropic",
            res.status().as_u16(),
            started.elapsed().as_millis(),
        );
        let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let response = forward_streaming_response(res).map_err(AppError::from)?;
        return Ok(CloudProxyResult {
            response,
            response_body: None,
            status,
            error_message: if status.is_success() {
                None
            } else {
                Some(status.to_string())
            },
        });
    }

    let status = StatusCode::from_u16(res.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let data: Value = res.json().await.map_err(map_reqwest_error)?;
    let text = data
        .get("content")
        .and_then(|c| c.get(0))
        .and_then(|p: &Value| p.get("text"))
        .and_then(|t: &Value| t.as_str())
        .unwrap_or("");

    let id = data
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("anthropic-{}", Uuid::new_v4()));
    let model_label = data
        .get("model")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| model.clone());

    let resp_body = json!({
        "id": id,
        "object": "chat.completion",
        "model": format!("anthropic:{}", model_label),
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": text},
        "finish_reason": "stop"
    }],
    });

    let built = Response::builder()
        .status(status)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(Body::from(resp_body.to_string()))
        .map_err(|e| AppError::from(LbError::Http(e.to_string())))?;

    info!(
        provider = "anthropic",
        model = %model_label,
        request_id = %req_id,
        latency_ms = started.elapsed().as_millis(),
        stream = false,
        status = %status,
        "cloud proxy complete (anthropic)"
    );

    cloud_metrics::record("anthropic", status.as_u16(), started.elapsed().as_millis());

    let error_message = if status.is_success() {
        None
    } else {
        serde_json::to_string(&data).ok()
    };

    Ok(CloudProxyResult {
        response: built,
        response_body: Some(resp_body),
        status,
        error_message,
    })
}

#[allow(clippy::too_many_arguments)]
async fn proxy_openai_cloud_post(
    state: &AppState,
    target_path: &str,
    model: &str,
    stream: bool,
    payload: Value,
    request_type: RequestType,
    client_ip: Option<IpAddr>,
    api_key_id: Option<Uuid>,
) -> Result<Response, AppError> {
    let (provider, model_name) = parse_cloud_model(model)
        .ok_or_else(|| validation_error("cloud model prefix is invalid"))?;
    let (endpoint_id, endpoint_name, endpoint_ip) = cloud_virtual_node(&provider);
    let record_id = Uuid::new_v4();
    let timestamp = Utc::now();
    let request_body = sanitize_openai_payload_for_history(&payload);
    let started = Instant::now();

    let outcome = match match provider.as_str() {
        "openai" => {
            proxy_openai_provider(&state.http_client, target_path, payload, stream, model_name)
                .await
        }
        "google" => proxy_google_provider(&state.http_client, model_name, payload, stream).await,
        "anthropic" => {
            proxy_anthropic_provider(&state.http_client, model_name, payload, stream).await
        }
        _ => Err(validation_error("unsupported cloud provider prefix")),
    } {
        Ok(res) => res,
        Err(e) => {
            let duration = started.elapsed();
            save_request_record(
                state.request_history.clone(),
                RequestResponseRecord {
                    id: record_id,
                    timestamp,
                    request_type,
                    model: model.to_string(),
                    endpoint_id,
                    endpoint_name,
                    endpoint_ip,
                    client_ip,
                    request_body,
                    response_body: None,
                    duration_ms: duration.as_millis() as u64,
                    status: RecordStatus::Error {
                        message: format!("{e:?}"),
                    },
                    completed_at: Utc::now(),
                    input_tokens: None,
                    output_tokens: None,
                    total_tokens: None,
                    api_key_id,
                },
            );
            return Err(e);
        }
    };

    let duration = started.elapsed();
    let status = outcome.status;
    let status_record = if status.is_success() {
        RecordStatus::Success
    } else {
        RecordStatus::Error {
            message: outcome
                .error_message
                .clone()
                .unwrap_or_else(|| status.to_string()),
        }
    };
    let response_body = if status.is_success() {
        outcome.response_body.clone()
    } else {
        None
    };

    save_request_record(
        state.request_history.clone(),
        RequestResponseRecord {
            id: record_id,
            timestamp,
            request_type,
            model: model.to_string(),
            endpoint_id,
            endpoint_name,
            endpoint_ip,
            client_ip,
            request_body,
            response_body,
            duration_ms: duration.as_millis() as u64,
            status: status_record,
            completed_at: Utc::now(),
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            api_key_id,
        },
    );

    Ok(outcome.response)
}

#[allow(deprecated)] // NodeRegistry migration in progress
#[allow(clippy::too_many_arguments)]
async fn proxy_openai_post(
    state: &AppState,
    payload: Value,
    target_path: &str,
    model: String,
    stream: bool,
    request_type: RequestType,
    client_ip: Option<IpAddr>,
    api_key_id: Option<Uuid>,
) -> Result<Response, AppError> {
    // Cloud-prefixed model -> forward to provider API
    if parse_cloud_model(&model).is_some() {
        return proxy_openai_cloud_post(
            state,
            target_path,
            &model,
            stream,
            payload,
            request_type,
            client_ip,
            api_key_id,
        )
        .await;
    }

    // Check if any endpoint has this model
    if state
        .endpoint_registry
        .find_by_model(&model)
        .await
        .is_empty()
    {
        let is_registered = load_registered_model(&state.db_pool, &model).await?;
        if is_registered.is_none() {
            let message = format!("The model '{}' does not exist", model);
            return Ok(openai_error_response(message, StatusCode::NOT_FOUND));
        }
    }

    let record_id = Uuid::new_v4();
    let timestamp = Utc::now();
    let request_body = sanitize_openai_payload_for_history(&payload);
    let tps_api_kind = TpsApiKind::from_request_type(request_type);
    let queue_config = state.queue_config;
    let mut queued_wait_ms: Option<u128> = None;

    // FR-004: エンドポイント選択失敗時もリクエスト履歴に記録する
    let endpoint =
        match select_available_endpoint_with_queue_for_model(state, queue_config, &model).await {
            Ok(QueueSelection::Ready {
                endpoint,
                queued_wait_ms: wait_ms,
            }) => {
                queued_wait_ms = wait_ms;
                *endpoint
            }
            Ok(QueueSelection::CapacityExceeded) => {
                let message = "Request queue is full".to_string();
                save_request_record(
                    state.request_history.clone(),
                    RequestResponseRecord {
                        id: record_id,
                        timestamp,
                        request_type,
                        model: model.clone(),
                        endpoint_id: Uuid::nil(),
                        endpoint_name: "N/A".to_string(),
                        endpoint_ip: UNSPECIFIED_IP,
                        client_ip,
                        request_body,
                        response_body: None,
                        duration_ms: 0,
                        status: RecordStatus::Error {
                            message: message.clone(),
                        },
                        completed_at: Utc::now(),
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                        api_key_id,
                    },
                );
                let retry_after = queue_config.timeout.as_secs().max(1);
                return Ok(queue_error_response(
                    StatusCode::TOO_MANY_REQUESTS,
                    &message,
                    "rate_limit_exceeded",
                    Some(retry_after),
                ));
            }
            Ok(QueueSelection::Timeout { waited_ms }) => {
                let message = "Queue wait timeout".to_string();
                save_request_record(
                    state.request_history.clone(),
                    RequestResponseRecord {
                        id: record_id,
                        timestamp,
                        request_type,
                        model: model.clone(),
                        endpoint_id: Uuid::nil(),
                        endpoint_name: "N/A".to_string(),
                        endpoint_ip: UNSPECIFIED_IP,
                        client_ip,
                        request_body,
                        response_body: None,
                        duration_ms: waited_ms as u64,
                        status: RecordStatus::Error {
                            message: message.clone(),
                        },
                        completed_at: Utc::now(),
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                        api_key_id,
                    },
                );
                return Ok(queue_error_response(
                    StatusCode::GATEWAY_TIMEOUT,
                    &message,
                    "timeout",
                    None,
                ));
            }
            Err(e) => {
                let error_message = if matches!(e, LbError::NoCapableEndpoints(_)) {
                    format!("No available nodes support model: {}", model)
                } else {
                    format!("Node selection failed: {}", e)
                };
                error!(
                    endpoint = %target_path,
                    model = %model,
                    error = %e,
                    "Failed to select available node"
                );
                save_request_record(
                    state.request_history.clone(),
                    RequestResponseRecord {
                        id: record_id,
                        timestamp,
                        request_type,
                        model: model.clone(),
                        endpoint_id: Uuid::nil(),
                        endpoint_name: "N/A".to_string(),
                        endpoint_ip: UNSPECIFIED_IP,
                        client_ip,
                        request_body,
                        response_body: None,
                        duration_ms: queued_wait_ms.unwrap_or(0) as u64,
                        status: RecordStatus::Error {
                            message: error_message.clone(),
                        },
                        completed_at: Utc::now(),
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                        api_key_id,
                    },
                );
                if matches!(e, LbError::NoCapableEndpoints(_)) {
                    return Ok(model_unavailable_response(
                        error_message,
                        "no_capable_nodes",
                    ));
                }
                return Err(e.into());
            }
        };
    let endpoint_id = endpoint.id;
    let endpoint_name = endpoint.name.clone();
    let endpoint_type = endpoint.endpoint_type;
    // RequestResponseRecordの互換性のため、デフォルトIP使用
    // (今後、RequestResponseRecordのフィールドをリネームすべき)
    let endpoint_host: std::net::IpAddr = UNSPECIFIED_IP;

    let request_lease = state
        .load_manager
        .begin_request(endpoint_id)
        .await
        .map_err(AppError::from)?;

    let client = state.http_client.clone();
    let runtime_url = format!("{}{}", endpoint.base_url.trim_end_matches('/'), target_path);
    let start = Instant::now();

    let mut request_builder = client.post(&runtime_url).json(&payload);
    if let Some(api_key) = &endpoint.api_key {
        request_builder = request_builder.bearer_auth(api_key);
    }

    let response = match request_builder.send().await {
        Ok(res) => res,
        Err(e) => {
            let duration = start.elapsed();
            request_lease
                .complete(RequestOutcome::Error, duration)
                .await
                .map_err(AppError::from)?;
            record_endpoint_request_stats(
                state.db_pool.clone(),
                endpoint_id,
                model.clone(),
                false,
                0,
                0,
                tps_api_kind,
                endpoint_type,
                state.load_manager.clone(),
                state.event_bus.clone(),
            );

            // Note: Model exclusion is handled by the health check system
            // which will mark the endpoint as offline/error if requests fail repeatedly

            save_request_record(
                state.request_history.clone(),
                RequestResponseRecord {
                    id: record_id,
                    timestamp,
                    request_type,
                    model: model.clone(),
                    endpoint_id,
                    endpoint_name: endpoint_name.clone(),
                    endpoint_ip: endpoint_host,
                    client_ip,
                    request_body: request_body.clone(),
                    response_body: None,
                    duration_ms: duration.as_millis() as u64,
                    status: RecordStatus::Error {
                        message: format!("Failed to proxy OpenAI request: {}", e),
                    },
                    completed_at: Utc::now(),
                    input_tokens: None,
                    output_tokens: None,
                    total_tokens: None,
                    api_key_id,
                },
            );

            return Err(LbError::Http(format!("Failed to proxy OpenAI request: {}", e)).into());
        }
    };

    // ストリームの場合はレスポンスをそのままパススルー
    if stream {
        let duration = start.elapsed();
        let succeeded = response.status().is_success();
        let outcome = if succeeded {
            RequestOutcome::Success
        } else {
            RequestOutcome::Error
        };
        request_lease
            .complete(outcome, duration)
            .await
            .map_err(AppError::from)?;
        if succeeded {
            // SPEC-f8e3a1b7: 成功時に推論レイテンシを更新
            update_inference_latency(&state.endpoint_registry, endpoint_id, duration);
        } else {
            record_endpoint_request_stats(
                state.db_pool.clone(),
                endpoint_id,
                model.clone(),
                false,
                0,
                0,
                tps_api_kind,
                endpoint_type,
                state.load_manager.clone(),
                state.event_bus.clone(),
            );
        }

        save_request_record(
            state.request_history.clone(),
            RequestResponseRecord {
                id: record_id,
                timestamp,
                request_type,
                model: model.clone(),
                endpoint_id,
                endpoint_name: endpoint_name.clone(),
                endpoint_ip: endpoint_host,
                client_ip,
                request_body: request_body.clone(),
                response_body: None, // ストリームボディは記録しない
                duration_ms: duration.as_millis() as u64,
                status: if succeeded {
                    RecordStatus::Success
                } else {
                    RecordStatus::Error {
                        message: format!("Upstream stream returned status {}", response.status()),
                    }
                },
                completed_at: Utc::now(),
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                api_key_id,
            },
        );

        let mut axum_response = if succeeded {
            forward_streaming_response_with_tps_tracking(
                response,
                endpoint_id,
                model.clone(),
                tps_api_kind,
                endpoint_type,
                start,
                state.db_pool.clone(),
                state.load_manager.clone(),
                state.event_bus.clone(),
            )
            .map_err(AppError::from)?
        } else {
            forward_streaming_response(response).map_err(AppError::from)?
        };
        if let Some(wait_ms) = queued_wait_ms {
            add_queue_headers(&mut axum_response, wait_ms);
        }
        return Ok(axum_response);
    }

    if !response.status().is_success() {
        let duration = start.elapsed();
        request_lease
            .complete(RequestOutcome::Error, duration)
            .await
            .map_err(AppError::from)?;
        record_endpoint_request_stats(
            state.db_pool.clone(),
            endpoint_id,
            model.clone(),
            false,
            0,
            0,
            tps_api_kind,
            endpoint_type,
            state.load_manager.clone(),
            state.event_bus.clone(),
        );

        // Note: Model exclusion is handled by the health check system
        // which will mark the endpoint as offline/error if requests fail repeatedly

        let status = response.status();
        // OpenAI互換経路では upstream 非2xx は 502 に正規化して返す
        let status_code = StatusCode::BAD_GATEWAY;
        let body_bytes = response.bytes().await.unwrap_or_default();
        let message = if body_bytes.is_empty() {
            status.to_string()
        } else {
            String::from_utf8_lossy(&body_bytes).trim().to_string()
        };

        save_request_record(
            state.request_history.clone(),
            RequestResponseRecord {
                id: record_id,
                timestamp,
                request_type,
                model: model.clone(),
                endpoint_id,
                endpoint_name: endpoint_name.clone(),
                endpoint_ip: endpoint_host,
                client_ip,
                request_body: request_body.clone(),
                response_body: None,
                duration_ms: duration.as_millis() as u64,
                status: RecordStatus::Error {
                    message: message.clone(),
                },
                completed_at: Utc::now(),
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                api_key_id,
            },
        );

        let payload = json!({
            "error": {
                "message": message,
                "type": "endpoint_upstream_error",
                "code": status_code.as_u16(),
            }
        });

        let mut response = (status_code, Json(payload)).into_response();
        if let Some(wait_ms) = queued_wait_ms {
            add_queue_headers(&mut response, wait_ms);
        }
        return Ok(response);
    }

    let parsed = response.json::<Value>().await;
    let duration = start.elapsed();

    match parsed {
        Ok(body) => {
            // レスポンスからトークン使用量を抽出
            let token_usage = extract_usage_from_response(&body);

            request_lease
                .complete_with_tokens(RequestOutcome::Success, duration, token_usage.clone())
                .await
                .map_err(AppError::from)?;
            // SPEC-f8e3a1b7: 成功時に推論レイテンシを更新
            update_inference_latency(&state.endpoint_registry, endpoint_id, duration);
            // SPEC-4bb5b55f: TPS計測用にoutput_tokensとdurationを渡す
            let tps_output_tokens = token_usage
                .as_ref()
                .and_then(|u| u.output_tokens)
                .unwrap_or(0) as u64;
            let tps_duration_ms = if tps_output_tokens > 0 {
                duration.as_millis().max(1) as u64
            } else {
                0
            };
            record_endpoint_request_stats(
                state.db_pool.clone(),
                endpoint_id,
                model.clone(),
                true,
                tps_output_tokens,
                tps_duration_ms,
                tps_api_kind,
                endpoint_type,
                state.load_manager.clone(),
                state.event_bus.clone(),
            );

            // RequestResponseRecordにトークン情報を保存
            let (input_tokens, output_tokens, total_tokens) = token_usage
                .as_ref()
                .map(|u| (u.input_tokens, u.output_tokens, u.total_tokens))
                .unwrap_or((None, None, None));

            save_request_record(
                state.request_history.clone(),
                RequestResponseRecord {
                    id: record_id,
                    timestamp,
                    request_type,
                    model,
                    endpoint_id,
                    endpoint_name,
                    endpoint_ip: endpoint_host,
                    client_ip,
                    request_body,
                    response_body: Some(body.clone()),
                    duration_ms: duration.as_millis() as u64,
                    status: RecordStatus::Success,
                    completed_at: Utc::now(),
                    input_tokens,
                    output_tokens,
                    total_tokens,
                    api_key_id,
                },
            );

            let mut response = (StatusCode::OK, Json(body)).into_response();
            if let Some(wait_ms) = queued_wait_ms {
                add_queue_headers(&mut response, wait_ms);
            }
            Ok(response)
        }
        Err(e) => {
            request_lease
                .complete(RequestOutcome::Error, duration)
                .await
                .map_err(AppError::from)?;
            record_endpoint_request_stats(
                state.db_pool.clone(),
                endpoint_id,
                model.clone(),
                false,
                0,
                0,
                tps_api_kind,
                endpoint_type,
                state.load_manager.clone(),
                state.event_bus.clone(),
            );

            // Note: Model exclusion is handled by the health check system
            // which will mark the endpoint as offline/error if requests fail repeatedly

            save_request_record(
                state.request_history.clone(),
                RequestResponseRecord {
                    id: record_id,
                    timestamp,
                    request_type,
                    model,
                    endpoint_id,
                    endpoint_name,
                    endpoint_ip: endpoint_host,
                    client_ip,
                    request_body,
                    response_body: None,
                    duration_ms: duration.as_millis() as u64,
                    status: RecordStatus::Error {
                        message: format!("Failed to parse OpenAI response: {}", e),
                    },
                    completed_at: Utc::now(),
                    input_tokens: None,
                    output_tokens: None,
                    total_tokens: None,
                    api_key_id,
                },
            );

            Err(LbError::Http(format!("Failed to parse OpenAI response: {}", e)).into())
        }
    }
}

#[allow(dead_code)]
async fn proxy_openai_get(state: &AppState, target_path: &str) -> Result<Response, AppError> {
    let endpoint = select_available_endpoint(state).await?;
    let endpoint_id = endpoint.id;

    let request_lease = state
        .load_manager
        .begin_request(endpoint_id)
        .await
        .map_err(AppError::from)?;

    let client = state.http_client.clone();
    let runtime_url = format!("{}{}", endpoint.base_url.trim_end_matches('/'), target_path);
    let start = Instant::now();

    let response = client.get(&runtime_url).send().await.map_err(|e| {
        AppError::from(LbError::Http(format!(
            "Failed to proxy OpenAI models request: {}",
            e
        )))
    })?;

    let duration = start.elapsed();
    let outcome = if response.status().is_success() {
        RequestOutcome::Success
    } else {
        RequestOutcome::Error
    };
    request_lease
        .complete(outcome, duration)
        .await
        .map_err(AppError::from)?;

    if !response.status().is_success() {
        let status = response.status();
        let status_code = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let body_bytes = response.bytes().await.unwrap_or_default();
        let message = if body_bytes.is_empty() {
            status.to_string()
        } else {
            String::from_utf8_lossy(&body_bytes).trim().to_string()
        };

        let payload = json!({
            "error": {
                "message": message,
                "type": "node_upstream_error",
                "code": status_code.as_u16(),
            }
        });

        return Ok((status_code, Json(payload)).into_response());
    }

    let body = response.json::<Value>().await.map_err(|e| {
        AppError::from(LbError::Http(format!(
            "Failed to parse OpenAI models response: {}",
            e
        )))
    })?;

    Ok((StatusCode::OK, Json(body)).into_response())
}

fn validation_error(message: impl Into<String>) -> AppError {
    let err = LbError::Common(CommonError::Validation(message.into()));
    err.into()
}

#[cfg(test)]
mod tests {
    use super::{
        extract_client_ip_from_headers, parse_client_ip_from_forwarded_value, parse_cloud_model,
        proxy_openai_cloud_post, proxy_openai_post,
    };
    use crate::common::protocol::{RecordStatus, RequestType};
    use crate::{
        balancer::LoadManager,
        db::{request_history::RequestHistoryStorage, test_utils::TEST_LOCK},
        AppState,
    };
    use axum::body::to_bytes;
    use axum::http::{HeaderMap, HeaderValue, StatusCode};
    use serde_json::json;
    use serial_test::serial;
    use sqlx::SqlitePool;
    use std::net::IpAddr;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::time::{sleep, Duration};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn create_local_state() -> AppState {
        let db_pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("sqlite memory connect");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("migrations");
        let request_history = Arc::new(RequestHistoryStorage::new(db_pool.clone()));
        let endpoint_registry = crate::registry::endpoints::EndpointRegistry::new(db_pool.clone())
            .await
            .expect("Failed to create endpoint registry");
        let endpoint_registry_arc = Arc::new(endpoint_registry.clone());
        let load_manager = LoadManager::new(endpoint_registry_arc);
        let http_client = reqwest::Client::new();
        let inference_gate = crate::inference_gate::InferenceGate::default();
        let shutdown = crate::shutdown::ShutdownController::default();
        let update_manager = crate::update::UpdateManager::new(
            http_client.clone(),
            inference_gate.clone(),
            shutdown.clone(),
        )
        .expect("Failed to create update manager");
        let audit_log_storage =
            std::sync::Arc::new(crate::db::audit_log::AuditLogStorage::new(db_pool.clone()));
        let audit_log_writer = crate::audit::writer::AuditLogWriter::new(
            crate::db::audit_log::AuditLogStorage::new(db_pool.clone()),
            crate::audit::writer::AuditLogWriterConfig::default(),
        );
        AppState {
            load_manager,
            request_history,
            db_pool,
            jwt_secret: "test-secret".into(),
            http_client,
            queue_config: crate::config::QueueConfig::from_env(),
            event_bus: crate::events::create_shared_event_bus(),
            endpoint_registry,
            inference_gate,
            shutdown,
            update_manager,
            audit_log_writer,
            audit_log_storage,
            audit_archive_pool: None,
        }
    }

    async fn create_state_with_tempdir() -> (AppState, tempfile::TempDir) {
        let dir = tempdir().expect("temp dir");
        std::env::set_var("LLMLB_DATA_DIR", dir.path());
        let state = create_local_state().await;
        (state, dir)
    }

    #[test]
    fn parse_cloud_prefixes() {
        assert_eq!(
            parse_cloud_model("openai:gpt-4o"),
            Some(("openai".to_string(), "gpt-4o".to_string()))
        );
        assert_eq!(
            parse_cloud_model("google:gemini-pro"),
            Some(("google".to_string(), "gemini-pro".to_string()))
        );
        assert_eq!(
            parse_cloud_model("ahtnorpic:claude-3"),
            Some(("anthropic".to_string(), "claude-3".to_string()))
        );
        assert_eq!(parse_cloud_model("gpt-4"), None);
        assert_eq!(parse_cloud_model("openai:"), None);
    }

    #[test]
    fn parse_client_ip_from_forwarded_value_supports_bracketed_ipv6_with_port() {
        let parsed = parse_client_ip_from_forwarded_value("\"[2001:db8::7]:4711\"")
            .expect("must parse bracketed ipv6");
        assert_eq!(parsed, "2001:db8::7".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn extract_client_ip_from_headers_prefers_first_valid_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("unknown, 203.0.113.10, 10.0.0.1"),
        );
        headers.insert(
            "forwarded",
            HeaderValue::from_static("for=198.51.100.20;proto=https"),
        );

        let parsed = extract_client_ip_from_headers(&headers).expect("must parse x-forwarded-for");
        assert_eq!(parsed, "203.0.113.10".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn extract_client_ip_from_headers_falls_back_to_forwarded() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "forwarded",
            HeaderValue::from_static("for=unknown;proto=https, for=\"[2001:db8::11]:8443\""),
        );

        let parsed = extract_client_ip_from_headers(&headers).expect("must parse forwarded");
        assert_eq!(parsed, "2001:db8::11".parse::<IpAddr>().unwrap());
    }

    #[tokio::test]
    #[serial]
    async fn openai_prefix_requires_api_key() {
        let _guard = TEST_LOCK.lock().await;
        // Save and remove any existing API key to test error case
        let saved = std::env::var("OPENAI_API_KEY").ok();
        std::env::remove_var("OPENAI_API_KEY");
        let (state, _dir) = create_state_with_tempdir().await;

        let payload = json!({"model":"openai:gpt-4o","messages":[]});
        let err = proxy_openai_cloud_post(
            &state,
            "/v1/chat/completions",
            "openai:gpt-4o",
            false,
            payload,
            RequestType::Chat,
            None,
            None,
        )
        .await
        .unwrap_err();
        let msg = format!("{:?}", err);
        assert!(
            msg.contains("OPENAI_API_KEY"),
            "expected error mentioning OPENAI_API_KEY, got {}",
            msg
        );

        // Restore API key if it was set
        if let Some(key) = saved {
            std::env::set_var("OPENAI_API_KEY", key);
        }
        std::env::remove_var("LLMLB_DATA_DIR");
    }

    #[tokio::test]
    #[serial]
    async fn google_prefix_requires_api_key() {
        let _guard = TEST_LOCK.lock().await;
        // Save and remove any existing API key to test error case
        let saved = std::env::var("GOOGLE_API_KEY").ok();
        std::env::remove_var("GOOGLE_API_KEY");
        let (state, _dir) = create_state_with_tempdir().await;

        let payload = json!({"model":"google:gemini-pro","messages":[]});
        let err = proxy_openai_cloud_post(
            &state,
            "/v1/chat/completions",
            "google:gemini-pro",
            false,
            payload,
            RequestType::Chat,
            None,
            None,
        )
        .await
        .unwrap_err();
        let msg = format!("{:?}", err);
        assert!(
            msg.contains("GOOGLE_API_KEY"),
            "expected GOOGLE_API_KEY error, got {}",
            msg
        );

        // Restore API key if it was set
        if let Some(key) = saved {
            std::env::set_var("GOOGLE_API_KEY", key);
        }
        std::env::remove_var("LLMLB_DATA_DIR");
    }

    #[tokio::test]
    #[serial]
    async fn anthropic_prefix_requires_api_key() {
        let _guard = TEST_LOCK.lock().await;
        // Save and remove any existing API key to test error case
        let saved = std::env::var("ANTHROPIC_API_KEY").ok();
        std::env::remove_var("ANTHROPIC_API_KEY");
        let (state, _dir) = create_state_with_tempdir().await;

        let payload = json!({"model":"anthropic:claude-3","messages":[]});
        let err = proxy_openai_cloud_post(
            &state,
            "/v1/chat/completions",
            "anthropic:claude-3",
            false,
            payload,
            RequestType::Chat,
            None,
            None,
        )
        .await
        .unwrap_err();
        let msg = format!("{:?}", err);
        assert!(
            msg.contains("ANTHROPIC_API_KEY"),
            "expected ANTHROPIC_API_KEY error, got {}",
            msg
        );

        // Restore API key if it was set
        if let Some(key) = saved {
            std::env::set_var("ANTHROPIC_API_KEY", key);
        }
        std::env::remove_var("LLMLB_DATA_DIR");
    }

    #[tokio::test]
    #[serial]
    async fn openai_prefix_streams_via_cloud() {
        let _guard = TEST_LOCK.lock().await;
        let server = MockServer::start().await;
        let tmpl = ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_raw(
                "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
                "text/event-stream",
            );
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(tmpl)
            .mount(&server)
            .await;

        std::env::set_var("OPENAI_API_KEY", "testkey");
        std::env::set_var("OPENAI_BASE_URL", server.uri());
        let (state, _dir) = create_state_with_tempdir().await;

        let payload = json!({"model":"openai:gpt-4o","messages":[],"stream":true});
        let resp = proxy_openai_cloud_post(
            &state,
            "/v1/chat/completions",
            "openai:gpt-4o",
            true,
            payload,
            RequestType::Chat,
            None,
            None,
        )
        .await
        .expect("cloud stream response");
        let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("delta"));
        assert!(body_str.contains("hi"));

        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("OPENAI_BASE_URL");
        std::env::remove_var("LLMLB_DATA_DIR");
    }

    #[tokio::test]
    #[serial]
    async fn google_prefix_proxies_and_maps_response() {
        let _guard = TEST_LOCK.lock().await;
        let server = MockServer::start().await;
        let tmpl = ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{"content": {"parts": [{"text": "hello from gemini"}]}}]
        }));
        Mock::given(method("POST"))
            .and(path("/models/gemini-pro:generateContent"))
            .respond_with(tmpl)
            .mount(&server)
            .await;

        std::env::set_var("GOOGLE_API_KEY", "gkey");
        std::env::set_var("GOOGLE_API_BASE_URL", server.uri());
        let (state, _dir) = create_state_with_tempdir().await;

        let payload =
            json!({"model":"google:gemini-pro","messages":[{"role":"user","content":"hi"}]});
        let resp = proxy_openai_cloud_post(
            &state,
            "/v1/chat/completions",
            "google:gemini-pro",
            false,
            payload,
            RequestType::Chat,
            None,
            None,
        )
        .await
        .expect("google mapped response");
        let bytes = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["model"].as_str().unwrap(), "google:gemini-pro");
        assert_eq!(
            v["choices"][0]["message"]["content"].as_str().unwrap(),
            "hello from gemini"
        );

        std::env::remove_var("GOOGLE_API_KEY");
        std::env::remove_var("GOOGLE_API_BASE_URL");
        std::env::remove_var("LLMLB_DATA_DIR");
    }

    #[tokio::test]
    #[serial]
    async fn anthropic_prefix_proxies_and_maps_response() {
        let _guard = TEST_LOCK.lock().await;
        let server = MockServer::start().await;
        let tmpl = ResponseTemplate::new(200).set_body_json(json!({
                "id": "abc123",
                "model": "claude-3",
            "content": [{"text": "anthropic says hi"}]
        }));
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(tmpl)
            .mount(&server)
            .await;

        std::env::set_var("ANTHROPIC_API_KEY", "akey");
        std::env::set_var("ANTHROPIC_API_BASE_URL", server.uri());
        let (state, _dir) = create_state_with_tempdir().await;

        let payload =
            json!({"model":"anthropic:claude-3","messages":[{"role":"user","content":"hi"}]});
        let resp = proxy_openai_cloud_post(
            &state,
            "/v1/chat/completions",
            "anthropic:claude-3",
            false,
            payload,
            RequestType::Chat,
            None,
            None,
        )
        .await
        .expect("anthropic mapped response");
        let bytes = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["model"].as_str().unwrap(), "anthropic:claude-3");
        assert_eq!(
            v["choices"][0]["message"]["content"].as_str().unwrap(),
            "anthropic says hi"
        );

        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("ANTHROPIC_API_BASE_URL");
        std::env::remove_var("LLMLB_DATA_DIR");
    }

    #[tokio::test]
    #[serial]
    async fn cloud_request_is_recorded_in_history() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempdir().expect("temp dir");
        std::env::set_var("LLMLB_DATA_DIR", temp_dir.path());

        let state = create_local_state().await;
        let server = MockServer::start().await;
        let tmpl = ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-123",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "hello"},
                "finish_reason": "stop"
            }]
        }));
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(tmpl)
            .mount(&server)
            .await;

        std::env::set_var("OPENAI_API_KEY", "testkey");
        std::env::set_var("OPENAI_BASE_URL", server.uri());

        let payload = json!({"model":"openai:gpt-4o","messages":[{"role":"user","content":"hi"}],"stream":false});
        let response = proxy_openai_post(
            &state,
            payload,
            "/v1/chat/completions",
            "openai:gpt-4o".into(),
            false,
            RequestType::Chat,
            None,
            None,
        )
        .await
        .expect("cloud proxy succeeds");

        assert_eq!(response.status(), StatusCode::OK);
        sleep(Duration::from_millis(20)).await;

        let records = state.request_history.load_records().await.expect("records");
        assert_eq!(records.len(), 1, "cloud request should be recorded");

        let record = &records[0];
        assert_eq!(record.model, "openai:gpt-4o");
        assert!(matches!(record.status, RecordStatus::Success));
        assert_eq!(record.request_type, RequestType::Chat);
        assert!(
            record.response_body.is_some(),
            "response should be captured"
        );

        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("OPENAI_BASE_URL");
        std::env::remove_var("LLMLB_DATA_DIR");
    }

    #[tokio::test]
    #[serial]
    async fn cloud_request_is_listed_in_dashboard_history() {
        use axum::routing::Router;
        use std::net::SocketAddr;
        use tokio::net::TcpListener;

        let _guard = TEST_LOCK.lock().await;

        // mock cloud provider
        let server = MockServer::start().await;
        let tmpl = ResponseTemplate::new(200).set_body_json(json!({
                "id": "chatcmpl-dashboard",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "hello cloud"},
                "finish_reason": "stop"
            }]
        }));
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(tmpl)
            .mount(&server)
            .await;

        // lb state with temp data dir
        std::env::set_var("OPENAI_API_KEY", "testkey");
        std::env::set_var("OPENAI_BASE_URL", server.uri());
        let (state, dir) = create_state_with_tempdir().await;

        // spawn lb
        let app: Router = crate::api::create_app(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr: SocketAddr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .ok();
        });

        // send cloud request
        let client = reqwest::Client::new();
        let payload = json!({"model":"openai:gpt-4o","messages":[{"role":"user","content":"hi"}]});
        let resp = client
            .post(format!("http://{addr}/v1/chat/completions"))
            .header("x-api-key", "sk_debug")
            .json(&payload)
            .send()
            .await
            .expect("send cloud request");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);

        // wait for async save_request_record
        tokio::time::sleep(Duration::from_millis(50)).await;

        // login (dashboard is JWT-only; API keys are rejected for /api/dashboard/*)
        let login_resp = client
            .post(format!("http://{addr}/api/auth/login"))
            .header("content-type", "application/json")
            .json(&json!({"username":"admin","password":"test"}))
            .send()
            .await
            .expect("login request");
        assert_eq!(login_resp.status(), reqwest::StatusCode::OK);
        let login_body: serde_json::Value = login_resp.json().await.expect("login json");
        let jwt_token = login_body["token"].as_str().expect("login token");

        // fetch dashboard history
        let history_resp = client
            .get(format!("http://{addr}/api/dashboard/request-responses"))
            .header("authorization", format!("Bearer {jwt_token}"))
            .send()
            .await
            .expect("history request");
        assert_eq!(history_resp.status(), reqwest::StatusCode::OK);
        let body: serde_json::Value = history_resp.json().await.expect("history json");
        let records = body["records"].as_array().expect("records array");
        assert!(
            records.iter().any(|r| r["model"] == "openai:gpt-4o"),
            "cloud request should be listed in history"
        );

        // cleanup env
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("OPENAI_BASE_URL");
        std::env::remove_var("LLMLB_DATA_DIR");
        drop(dir);
    }

    #[tokio::test]
    #[serial]
    async fn non_prefixed_model_stays_on_local_path() {
        let _guard = TEST_LOCK.lock().await;
        let state = create_local_state().await;
        let payload = json!({"model":"gpt-oss-20b","messages":[]});
        let res = proxy_openai_post(
            &state,
            payload,
            "/v1/chat/completions",
            "gpt-oss-20b".into(),
            false,
            RequestType::Chat,
            None,
            None,
        )
        .await;
        // モデルが登録されておらず、どのノードも報告していない場合は404
        let response = res.expect("expected 404 response, not Err");
        assert_eq!(
            response.status(),
            axum::http::StatusCode::NOT_FOUND,
            "expected NOT_FOUND for unregistered model"
        );
    }

    #[tokio::test]
    #[serial]
    async fn direct_routing_body_read_failure_releases_active_request() {
        use crate::types::endpoint::{
            Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
        };
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let _guard = TEST_LOCK.lock().await;
        let (state, _dir) = create_state_with_tempdir().await;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut read_buf = [0u8; 4096];
                let _ = socket.read(&mut read_buf).await;
                // Intentionally send fewer bytes than Content-Length to force body read failure.
                let response = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 256\r\nConnection: close\r\n\r\n{\"id\":\"truncated\"}";
                let _ = socket.write_all(response).await;
                let _ = socket.shutdown().await;
            }
        });

        let mut endpoint = Endpoint::new(
            "broken-endpoint".to_string(),
            format!("http://{addr}"),
            EndpointType::OpenaiCompatible,
        );
        endpoint.status = EndpointStatus::Online;
        let endpoint_id = endpoint.id;
        state
            .endpoint_registry
            .add(endpoint)
            .await
            .expect("add endpoint");
        state
            .endpoint_registry
            .add_model(&EndpointModel {
                endpoint_id,
                model_id: "broken-model".to_string(),
                capabilities: None,
                max_tokens: None,
                last_checked: None,
                supported_apis: vec![SupportedAPI::ChatCompletions],
            })
            .await
            .expect("add endpoint model");

        let payload = json!({
            "model": "broken-model",
            "messages": [{"role":"user","content":"hello"}]
        });
        let result = proxy_openai_post(
            &state,
            payload,
            "/v1/chat/completions",
            "broken-model".to_string(),
            false,
            RequestType::Chat,
            None,
            None,
        )
        .await;

        assert!(
            result.is_err(),
            "expected upstream body read failure to return error"
        );

        let snapshot = state
            .load_manager
            .snapshot(endpoint_id)
            .await
            .expect("snapshot");
        assert_eq!(
            snapshot.active_requests, 0,
            "active request count must be released on body read error"
        );
    }

    #[tokio::test]
    #[serial]
    async fn local_streaming_request_updates_model_tps_after_stream_completion() {
        use crate::types::endpoint::{
            Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
        };

        let _guard = TEST_LOCK.lock().await;
        let (state, _dir) = create_state_with_tempdir().await;

        let server = MockServer::start().await;
        let stream_body = concat!(
            "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        let tmpl = ResponseTemplate::new(200)
            .insert_header("content-type", "text/event-stream")
            .set_body_raw(stream_body, "text/event-stream");
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(tmpl)
            .mount(&server)
            .await;

        let mut endpoint = Endpoint::new(
            "stream-tps-endpoint".to_string(),
            server.uri(),
            EndpointType::Vllm,
        );
        endpoint.status = EndpointStatus::Online;
        let endpoint_id = endpoint.id;
        state
            .endpoint_registry
            .add(endpoint)
            .await
            .expect("add endpoint");
        state
            .endpoint_registry
            .add_model(&EndpointModel {
                endpoint_id,
                model_id: "stream-tps-model".to_string(),
                capabilities: None,
                max_tokens: None,
                last_checked: None,
                supported_apis: vec![SupportedAPI::ChatCompletions],
            })
            .await
            .expect("add endpoint model");

        let payload = json!({
            "model": "stream-tps-model",
            "messages": [{"role":"user","content":"hello"}],
            "stream": true
        });
        let response = proxy_openai_post(
            &state,
            payload,
            "/v1/chat/completions",
            "stream-tps-model".to_string(),
            true,
            RequestType::Chat,
            None,
            None,
        )
        .await
        .expect("streaming request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(response.into_body(), 1_000_000)
            .await
            .expect("stream body should be readable");

        sleep(Duration::from_millis(100)).await;

        let tps = state.load_manager.get_model_tps(endpoint_id).await;
        let entry = tps
            .iter()
            .find(|info| info.model_id == "stream-tps-model")
            .expect("stream model should have TPS entry");
        assert!(entry.tps.is_some(), "TPS should be updated");
        assert!(
            entry.total_output_tokens > 0,
            "streaming output tokens should be accumulated"
        );
    }

    #[tokio::test]
    #[serial]
    async fn interrupted_streaming_request_still_records_success_stats() {
        use crate::types::endpoint::{
            Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
        };

        let _guard = TEST_LOCK.lock().await;
        let (state, _dir) = create_state_with_tempdir().await;

        let server = MockServer::start().await;
        let stream_body = concat!(
            "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
            "data: {\"id\":\"chatcmpl-123\",\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(stream_body, "text/event-stream"),
            )
            .mount(&server)
            .await;

        let mut endpoint = Endpoint::new(
            "stream-interrupted-endpoint".to_string(),
            server.uri(),
            EndpointType::Vllm,
        );
        endpoint.status = EndpointStatus::Online;
        let endpoint_id = endpoint.id;
        state
            .endpoint_registry
            .add(endpoint)
            .await
            .expect("add endpoint");
        state
            .endpoint_registry
            .add_model(&EndpointModel {
                endpoint_id,
                model_id: "stream-interrupted-model".to_string(),
                capabilities: None,
                max_tokens: None,
                last_checked: None,
                supported_apis: vec![SupportedAPI::ChatCompletions],
            })
            .await
            .expect("add endpoint model");

        let response = proxy_openai_post(
            &state,
            json!({
                "model": "stream-interrupted-model",
                "messages": [{"role":"user","content":"hello"}],
                "stream": true
            }),
            "/v1/chat/completions",
            "stream-interrupted-model".to_string(),
            true,
            RequestType::Chat,
            None,
            None,
        )
        .await
        .expect("streaming request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        // Simulate client disconnect before fully draining the upstream stream.
        drop(response);

        sleep(Duration::from_millis(120)).await;

        let endpoint = crate::db::endpoints::get_endpoint(&state.db_pool, endpoint_id)
            .await
            .expect("get endpoint should succeed")
            .expect("endpoint should exist");
        assert_eq!(endpoint.total_requests, 1);
        assert_eq!(endpoint.successful_requests, 1);
        assert_eq!(endpoint.failed_requests, 0);

        let model_stats =
            crate::db::endpoint_daily_stats::get_model_stats(&state.db_pool, endpoint_id)
                .await
                .expect("get model stats");
        let stat = model_stats
            .iter()
            .find(|s| s.model_id == "stream-interrupted-model")
            .expect("model stats should exist for interrupted stream");
        assert_eq!(stat.total_requests, 1);
        assert_eq!(stat.successful_requests, 1);
        assert_eq!(stat.failed_requests, 0);
    }

    #[tokio::test]
    #[serial]
    async fn non_stream_without_usage_does_not_accumulate_tps_duration() {
        use crate::types::endpoint::{
            Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI,
        };

        let _guard = TEST_LOCK.lock().await;
        let (state, _dir) = create_state_with_tempdir().await;

        let server = MockServer::start().await;
        let body_without_usage = json!({
            "id": "chatcmpl-no-usage",
            "object": "chat.completion",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "hello"},
                "finish_reason": "stop"
            }]
        });
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body_without_usage))
            .mount(&server)
            .await;

        let mut endpoint = Endpoint::new(
            "no-usage-endpoint".to_string(),
            server.uri(),
            EndpointType::Vllm,
        );
        endpoint.status = EndpointStatus::Online;
        let endpoint_id = endpoint.id;
        state
            .endpoint_registry
            .add(endpoint)
            .await
            .expect("add endpoint");
        state
            .endpoint_registry
            .add_model(&EndpointModel {
                endpoint_id,
                model_id: "no-usage-model".to_string(),
                capabilities: None,
                max_tokens: None,
                last_checked: None,
                supported_apis: vec![SupportedAPI::ChatCompletions],
            })
            .await
            .expect("add endpoint model");

        let payload = json!({
            "model": "no-usage-model",
            "messages": [{"role":"user","content":"hello"}],
            "stream": false
        });
        let response = proxy_openai_post(
            &state,
            payload,
            "/v1/chat/completions",
            "no-usage-model".to_string(),
            false,
            RequestType::Chat,
            None,
            None,
        )
        .await
        .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        sleep(Duration::from_millis(100)).await;

        let model_stats =
            crate::db::endpoint_daily_stats::get_model_stats(&state.db_pool, endpoint_id)
                .await
                .expect("get model stats");
        let stat = model_stats
            .iter()
            .find(|s| s.model_id == "no-usage-model")
            .expect("model stats should exist");
        assert_eq!(
            stat.total_output_tokens, 0,
            "usageがない場合はoutput_tokensを加算しない"
        );
        assert_eq!(
            stat.total_duration_ms, 0,
            "usageがない場合はduration_msを加算しない"
        );
    }

    #[tokio::test]
    #[serial]
    async fn streaming_allowed_for_cloud_prefix() {
        let _guard = TEST_LOCK.lock().await;
        // Save and remove any existing API key to test error case
        let saved = std::env::var("OPENAI_API_KEY").ok();
        std::env::remove_var("OPENAI_API_KEY");
        let (state, _dir) = create_state_with_tempdir().await;

        let payload = json!({"model":"openai:gpt-4o","messages":[],"stream":true});
        let err = proxy_openai_cloud_post(
            &state,
            "/v1/chat/completions",
            "openai:gpt-4o",
            true,
            payload,
            RequestType::Chat,
            None,
            None,
        )
        .await
        .unwrap_err();
        let msg = format!("{:?}", err);
        assert!(
            msg.contains("OPENAI_API_KEY"),
            "expected API key error (stream path), got {}",
            msg
        );

        // Restore API key if it was set
        if let Some(key) = saved {
            std::env::set_var("OPENAI_API_KEY", key);
        }
        std::env::remove_var("LLMLB_DATA_DIR");
    }

    // T006: chat capabilities検証テスト (RED)
    // TextGeneration capability を持たないモデルで /v1/chat/completions を呼ぶとエラー
    #[test]
    fn test_chat_capability_validation_error_message() {
        use crate::types::model::{ModelCapability, ModelType};

        // TTSモデルはTextToSpeechのみ、TextGenerationは非対応
        let tts_caps = ModelCapability::from_model_type(ModelType::TextToSpeech);
        assert!(!tts_caps.contains(&ModelCapability::TextGeneration));

        // ASRモデルもSpeechToTextのみ、TextGenerationは非対応
        let stt_caps = ModelCapability::from_model_type(ModelType::SpeechToText);
        assert!(!stt_caps.contains(&ModelCapability::TextGeneration));

        // EmbeddingモデルもEmbeddingのみ、TextGenerationは非対応
        let embed_caps = ModelCapability::from_model_type(ModelType::Embedding);
        assert!(!embed_caps.contains(&ModelCapability::TextGeneration));

        // 期待されるエラーメッセージ形式
        let model_name = "whisper-large-v3";
        let expected_error = format!("Model '{}' does not support text generation", model_name);
        assert!(expected_error.contains("does not support text generation"));
    }
}
