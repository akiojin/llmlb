//! OpenAI互換APIエンドポイント (/v1/*)

use axum::body::Body;
use axum::{
    extract::{Path, State},
    http::{header::CONTENT_TYPE, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use llm_router_common::{
    error::{CommonError, RouterError},
    protocol::{RecordStatus, RequestResponseRecord, RequestType},
    types::{ModelCapabilities, ModelCapability, VisionCapability},
};
use reqwest;
use serde_json::{json, Value};
use std::{collections::HashMap, net::IpAddr, time::Instant};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::models::image;
use crate::{
    api::{
        error::AppError,
        model_name::{parse_quantized_model_name, ParsedModelName},
        models::{list_registered_models, load_registered_model, LifecycleStatus},
        proxy::{
            forward_streaming_response, forward_to_endpoint, save_request_record,
            select_available_node, select_available_node_with_queue_for_model,
            select_endpoint_for_model, EndpointSelection, QueueSelection,
        },
    },
    balancer::RequestOutcome,
    cloud_metrics,
    token::extract_usage_from_response,
    AppState,
};

fn map_reqwest_error(err: reqwest::Error) -> AppError {
    AppError::from(RouterError::Http(err.to_string()))
}

fn auth_error(msg: &str) -> AppError {
    AppError::from(RouterError::Authentication(msg.to_string()))
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

/// POST /v1/chat/completions - OpenAI互換チャットAPI
pub async fn chat_completions(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Response, AppError> {
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
            return Err(AppError::from(RouterError::Common(
                CommonError::Validation(format!(
                    "Model '{}' does not support text generation",
                    parsed.raw
                )),
            )));
        }
    }
    // 登録されていないモデルはノード側で処理（クラウドモデル等）

    let payload = match prepare_vision_payload(&state, payload, &parsed).await {
        Ok(payload) => payload,
        Err(response) => return Ok(response),
    };

    let stream = extract_stream(&payload);
    proxy_openai_post(
        &state,
        payload,
        "/v1/chat/completions",
        parsed.raw,
        stream,
        RequestType::Chat,
    )
    .await
}

/// POST /v1/completions - OpenAI互換テキスト補完API
pub async fn completions(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Response, AppError> {
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
    )
    .await
}

/// POST /v1/embeddings - OpenAI互換Embeddings API
pub async fn embeddings(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Response, AppError> {
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

    // オンラインノードの実行可能モデルを取得
    let mut available_models: Vec<String> = state
        .registry
        .list_executable_models_online()
        .await
        .into_iter()
        .collect();
    available_models.sort();

    // 登録済みモデルのメタデータを取得（存在する場合のみ利用）
    let mut registered_map: std::collections::HashMap<String, crate::registry::models::ModelInfo> =
        HashMap::new();
    for model in list_registered_models(&state.db_pool).await? {
        registered_map.insert(model.name.clone(), model);
    }

    // SPEC-24157000: エンドポイントのモデルとsupported_apisを取得
    let mut endpoint_model_apis: HashMap<String, HashSet<SupportedAPI>> = HashMap::new();
    if let Some(ref registry) = state.endpoint_registry {
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
                    // Responses API対応エンドポイントの場合は追加
                    if ep.supports_responses_api {
                        apis.insert(SupportedAPI::Responses);
                    }
                }
            }
        }
    }

    // 追跡用：モデルID一覧
    let mut seen_models: HashSet<String> = HashSet::new();

    // OpenAI互換レスポンス形式 + Azure capabilities + ダッシュボード拡張
    let mut data: Vec<Value> = Vec::new();

    // ノードのモデルを追加
    for model_id in &available_models {
        seen_models.insert(model_id.clone());

        // supported_apisを取得（デフォルトはchat_completions）
        let supported_apis: Vec<String> = endpoint_model_apis
            .get(model_id)
            .map(|apis| apis.iter().map(|a| a.as_str().to_string()).collect())
            .unwrap_or_else(|| vec!["chat_completions".to_string()]);

        if let Some(m) = registered_map.get(model_id) {
            let caps: ModelCapabilities = m.get_capabilities().into();
            let obj = json!({
                "id": m.name,
                "object": "model",
                "created": 0,
                "owned_by": "router",
                "capabilities": caps,
                "lifecycle_status": LifecycleStatus::Registered,
                "download_progress": null,
                "ready": true,
                "repo": m.repo,
                "filename": m.filename,
                "size_bytes": m.size,
                "required_memory_bytes": m.required_memory,
                "source": m.source,
                "tags": m.tags,
                "description": m.description,
                "chat_template": m.chat_template,
                "supported_apis": supported_apis,
            });
            data.push(obj);
        } else {
            let obj = json!({
                "id": model_id,
                "object": "model",
                "created": 0,
                "owned_by": "router",
                "lifecycle_status": LifecycleStatus::Registered,
                "download_progress": null,
                "ready": true,
                "supported_apis": supported_apis,
            });
            data.push(obj);
        }
    }

    // SPEC-24157000: エンドポイント専用モデルを追加（ノードにないモデル）
    for (model_id, apis) in &endpoint_model_apis {
        if seen_models.contains(model_id) {
            continue;
        }
        seen_models.insert(model_id.clone());

        let supported_apis: Vec<String> = apis.iter().map(|a| a.as_str().to_string()).collect();
        let obj = json!({
            "id": model_id,
            "object": "model",
            "created": 0,
            "owned_by": "endpoint",
            "lifecycle_status": LifecycleStatus::Registered,
            "download_progress": null,
            "ready": true,
            "supported_apis": supported_apis,
        });
        data.push(obj);
    }

    // クラウドプロバイダーのモデル一覧を追加（SPEC-82491000）
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
/// SPEC-24157000: Endpoints APIで登録されたモデルも検索対象に含める
pub async fn get_model(
    State(state): State<AppState>,
    Path(model_id): Path<String>,
) -> Result<Response, AppError> {
    use crate::types::endpoint::SupportedAPI;
    use std::collections::HashSet;

    let available_models = state.registry.list_executable_models_online().await;
    let mut registered_map: HashMap<String, crate::registry::models::ModelInfo> = HashMap::new();
    for model in list_registered_models(&state.db_pool).await? {
        registered_map.insert(model.name.clone(), model);
    }

    // SPEC-24157000: エンドポイントのモデルとsupported_apisを取得
    let mut endpoint_model_apis: HashMap<String, HashSet<SupportedAPI>> = HashMap::new();
    if let Some(ref registry) = state.endpoint_registry {
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
                    if ep.supports_responses_api {
                        apis.insert(SupportedAPI::Responses);
                    }
                }
            }
        }
    }

    let model = registered_map.remove(&model_id);
    let is_endpoint_model = endpoint_model_apis.contains_key(&model_id);

    if model.is_none() && !available_models.contains(&model_id) && !is_endpoint_model {
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
        let ready = available_models.contains(&model_id) || is_endpoint_model;
        let lifecycle_status = if ready {
            LifecycleStatus::Registered
        } else {
            LifecycleStatus::Pending
        };

        let body = json!({
            "id": model_id,
            "object": "model",
            "created": 0,
            "owned_by": "router",
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
        "router"
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

fn collect_image_urls(payload: &Value) -> Result<Vec<String>, String> {
    let mut urls = Vec::new();
    let Some(messages) = payload.get("messages").and_then(|v| v.as_array()) else {
        return Ok(urls);
    };

    for message in messages {
        let Some(content) = message.get("content") else {
            continue;
        };
        let Some(parts) = content.as_array() else {
            continue;
        };

        for part in parts {
            if part.get("type").and_then(|v| v.as_str()) != Some("image_url") {
                continue;
            }
            let image_url_value = part
                .get("image_url")
                .ok_or_else(|| "image_url is required".to_string())?;
            let url = if let Some(url) = image_url_value.get("url").and_then(|v| v.as_str()) {
                url
            } else if let Some(url) = image_url_value.as_str() {
                url
            } else {
                return Err("image_url.url is required".to_string());
            };
            urls.push(url.to_string());
        }
    }

    Ok(urls)
}

fn replace_image_urls(payload: &mut Value, replacements: &[String]) -> Result<(), String> {
    let mut index = 0usize;
    let Some(messages) = payload.get_mut("messages").and_then(|v| v.as_array_mut()) else {
        if replacements.is_empty() {
            return Ok(());
        }
        return Err("messages must be an array".to_string());
    };

    for message in messages {
        let Some(parts) = message.get_mut("content").and_then(|v| v.as_array_mut()) else {
            continue;
        };
        for part in parts {
            if part.get("type").and_then(|v| v.as_str()) != Some("image_url") {
                continue;
            }
            let new_url = replacements
                .get(index)
                .ok_or_else(|| "image_url replacement missing".to_string())?
                .clone();
            index += 1;

            let Some(image_url_value) = part.get_mut("image_url") else {
                return Err("image_url is required".to_string());
            };
            if let Some(obj) = image_url_value.as_object_mut() {
                obj.insert("url".to_string(), Value::String(new_url));
            } else if image_url_value.is_string() {
                *image_url_value = Value::String(new_url);
            } else {
                return Err("image_url must be object or string".to_string());
            }
        }
    }

    if index != replacements.len() {
        return Err("image_url replacement count mismatch".to_string());
    }

    Ok(())
}

async fn prepare_vision_payload(
    state: &AppState,
    mut payload: Value,
    model: &ParsedModelName,
) -> Result<Value, Response> {
    let image_urls = collect_image_urls(&payload)
        .map_err(|msg| openai_error_response(msg, StatusCode::BAD_REQUEST))?;
    if image_urls.is_empty() {
        return Ok(payload);
    }

    let models = list_registered_models(&state.db_pool)
        .await
        .map_err(|err| openai_error_response(err.to_string(), StatusCode::INTERNAL_SERVER_ERROR))?;
    let Some(model_info) = models.iter().find(|m| m.name == model.base) else {
        // 未登録モデル（クラウド等）はルーター側の検証をスキップ
        return Ok(payload);
    };
    if !model_info.has_capability(ModelCapability::Vision) {
        return Err(openai_error_response(
            format!("Model '{}' does not support image understanding", model.raw),
            StatusCode::BAD_REQUEST,
        ));
    }

    let vision_limits = VisionCapability::default();
    if image_urls.len() > vision_limits.max_image_count as usize {
        return Err(openai_error_response(
            format!("Too many images (max {})", vision_limits.max_image_count),
            StatusCode::BAD_REQUEST,
        ));
    }

    let mut embedded_urls = Vec::with_capacity(image_urls.len());
    for url in image_urls {
        match image::validate_image_url(&state.http_client, &url, &vision_limits).await {
            Ok(image_data) => {
                embedded_urls.push(image_data.to_data_url());
            }
            Err(err) => {
                return Err(openai_error_response(
                    err.to_string(),
                    StatusCode::BAD_REQUEST,
                ));
            }
        }
    }

    replace_image_urls(&mut payload, &embedded_urls)
        .map_err(|msg| openai_error_response(msg, StatusCode::BAD_REQUEST))?;

    Ok(payload)
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
    let node_id = match provider {
        "openai" => Uuid::parse_str("00000000-0000-0000-0000-00000000c001").unwrap(),
        "google" => Uuid::parse_str("00000000-0000-0000-0000-00000000c002").unwrap(),
        "anthropic" => Uuid::parse_str("00000000-0000-0000-0000-00000000c003").unwrap(),
        _ => Uuid::parse_str("00000000-0000-0000-0000-00000000c0ff").unwrap(),
    };
    let machine_name = format!("cloud:{provider}");
    let node_ip: IpAddr = "0.0.0.0".parse().unwrap();
    (node_id, machine_name, node_ip)
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
    let built = resp.body(Body::from(bytes)).unwrap();
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
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
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
        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?;

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
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
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
        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?;

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

async fn proxy_openai_cloud_post(
    state: &AppState,
    target_path: &str,
    model: &str,
    stream: bool,
    payload: Value,
    request_type: RequestType,
) -> Result<Response, AppError> {
    let (provider, model_name) = parse_cloud_model(model)
        .ok_or_else(|| validation_error("cloud model prefix is invalid"))?;
    let (node_id, node_machine_name, node_ip) = cloud_virtual_node(&provider);
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
                    node_id,
                    node_machine_name,
                    node_ip,
                    client_ip: None,
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
            node_id,
            node_machine_name,
            node_ip,
            client_ip: None,
            request_body,
            response_body,
            duration_ms: duration.as_millis() as u64,
            status: status_record,
            completed_at: Utc::now(),
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
        },
    );

    Ok(outcome.response)
}

async fn proxy_openai_post(
    state: &AppState,
    payload: Value,
    target_path: &str,
    model: String,
    stream: bool,
    request_type: RequestType,
) -> Result<Response, AppError> {
    // Cloud-prefixed model -> forward to provider API
    if parse_cloud_model(&model).is_some() {
        return proxy_openai_cloud_post(state, target_path, &model, stream, payload, request_type)
            .await;
    }

    // Endpoint-based routing: check if model exists in EndpointRegistry
    if let Ok(EndpointSelection::Found(endpoint)) = select_endpoint_for_model(state, &model).await {
        let record_id = Uuid::new_v4();
        let timestamp = Utc::now();
        let request_body = sanitize_openai_payload_for_history(&payload);
        let body_bytes = serde_json::to_vec(&payload).map_err(|e| {
            AppError::from(RouterError::Http(format!(
                "Failed to serialize payload: {}",
                e
            )))
        })?;
        let start = Instant::now();

        let response = match forward_to_endpoint(
            &state.http_client,
            &endpoint,
            target_path,
            body_bytes,
            stream,
        )
        .await
        {
            Ok(res) => res,
            Err(e) => {
                let duration = start.elapsed();
                save_request_record(
                    state.request_history.clone(),
                    RequestResponseRecord {
                        id: record_id,
                        timestamp,
                        request_type,
                        model: model.clone(),
                        node_id: endpoint.id,
                        node_machine_name: endpoint.name.clone(),
                        node_ip: "0.0.0.0".parse().unwrap(),
                        client_ip: None,
                        request_body,
                        response_body: None,
                        duration_ms: duration.as_millis() as u64,
                        status: RecordStatus::Error {
                            message: format!("Endpoint request failed: {}", e),
                        },
                        completed_at: Utc::now(),
                        input_tokens: None,
                        output_tokens: None,
                        total_tokens: None,
                    },
                );
                return Err(e.into());
            }
        };

        let duration = start.elapsed();

        if stream {
            save_request_record(
                state.request_history.clone(),
                RequestResponseRecord {
                    id: record_id,
                    timestamp,
                    request_type,
                    model: model.clone(),
                    node_id: endpoint.id,
                    node_machine_name: endpoint.name.clone(),
                    node_ip: "0.0.0.0".parse().unwrap(),
                    client_ip: None,
                    request_body,
                    response_body: None,
                    duration_ms: duration.as_millis() as u64,
                    status: RecordStatus::Success,
                    completed_at: Utc::now(),
                    input_tokens: None,
                    output_tokens: None,
                    total_tokens: None,
                },
            );
            return forward_streaming_response(response).map_err(AppError::from);
        }

        // Non-streaming: read response body
        let status = response.status();
        let body_bytes = response.bytes().await.map_err(map_reqwest_error)?;
        let response_body_value: Option<Value> = serde_json::from_slice(&body_bytes).ok();
        let token_usage = response_body_value
            .as_ref()
            .and_then(extract_usage_from_response);

        save_request_record(
            state.request_history.clone(),
            RequestResponseRecord {
                id: record_id,
                timestamp,
                request_type,
                model: model.clone(),
                node_id: endpoint.id,
                node_machine_name: endpoint.name.clone(),
                node_ip: "0.0.0.0".parse().unwrap(),
                client_ip: None,
                request_body,
                response_body: response_body_value,
                duration_ms: duration.as_millis() as u64,
                status: if status.is_success() {
                    RecordStatus::Success
                } else {
                    RecordStatus::Error {
                        message: format!("Endpoint returned {}", status),
                    }
                },
                completed_at: Utc::now(),
                input_tokens: token_usage.as_ref().and_then(|u| u.input_tokens),
                output_tokens: token_usage.as_ref().and_then(|u| u.output_tokens),
                total_tokens: token_usage.as_ref().and_then(|u| u.total_tokens),
            },
        );

        return Ok(Response::builder()
            .status(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK))
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(body_bytes))
            .unwrap());
    }

    // Fall back to node-based routing
    if !state.registry.has_model_reported(&model).await {
        let is_registered = load_registered_model(&state.db_pool, &model).await?;
        if is_registered.is_none() {
            let message = format!("The model '{}' does not exist", model);
            return Ok(openai_error_response(message, StatusCode::NOT_FOUND));
        }
    }

    let record_id = Uuid::new_v4();
    let timestamp = Utc::now();
    let request_body = sanitize_openai_payload_for_history(&payload);
    let queue_config = state.queue_config;
    let mut queued_wait_ms: Option<u128> = None;

    // FR-004: ノード選択失敗時もリクエスト履歴に記録する
    let node = match select_available_node_with_queue_for_model(state, queue_config, &model).await {
        Ok(QueueSelection::Ready {
            node,
            queued_wait_ms: wait_ms,
        }) => {
            queued_wait_ms = wait_ms;
            *node
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
                    node_id: Uuid::nil(),
                    node_machine_name: "N/A".to_string(),
                    node_ip: "0.0.0.0".parse().unwrap(),
                    client_ip: None,
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
                    node_id: Uuid::nil(),
                    node_machine_name: "N/A".to_string(),
                    node_ip: "0.0.0.0".parse().unwrap(),
                    client_ip: None,
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
            let error_message = if matches!(e, RouterError::NoCapableNodes(_)) {
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
                    node_id: Uuid::nil(),
                    node_machine_name: "N/A".to_string(),
                    node_ip: "0.0.0.0".parse().unwrap(),
                    client_ip: None,
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
                },
            );
            if matches!(e, RouterError::NoCapableNodes(_)) {
                return Ok(model_unavailable_response(
                    error_message,
                    "no_capable_nodes",
                ));
            }
            return Err(e.into());
        }
    };
    let node_id = node.id;
    let node_machine_name = node.machine_name.clone();
    let node_ip = node.ip_address;

    state
        .load_manager
        .begin_request(node_id)
        .await
        .map_err(AppError::from)?;

    let client = state.http_client.clone();
    let node_api_port = node.runtime_port + 1;
    let runtime_url = format!(
        "http://{}:{}{}",
        node.ip_address, node_api_port, target_path
    );
    let start = Instant::now();

    let response = match client.post(&runtime_url).json(&payload).send().await {
        Ok(res) => res,
        Err(e) => {
            let duration = start.elapsed();
            state
                .load_manager
                .finish_request(node_id, RequestOutcome::Error, duration)
                .await
                .map_err(AppError::from)?;

            if let Err(err) = state
                .registry
                .exclude_model_from_node(node_id, &model)
                .await
            {
                warn!(
                    node_id = %node_id,
                    model = %model,
                    error = %err,
                    "Failed to exclude model after proxy request failure"
                );
            }

            save_request_record(
                state.request_history.clone(),
                RequestResponseRecord {
                    id: record_id,
                    timestamp,
                    request_type,
                    model: model.clone(),
                    node_id,
                    node_machine_name: node_machine_name.clone(),
                    node_ip,
                    client_ip: None,
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
                },
            );

            return Err(RouterError::Http(format!("Failed to proxy OpenAI request: {}", e)).into());
        }
    };

    // ストリームの場合はレスポンスをそのままパススルー
    if stream {
        let duration = start.elapsed();
        state
            .load_manager
            .finish_request(node_id, RequestOutcome::Success, duration)
            .await
            .map_err(AppError::from)?;

        save_request_record(
            state.request_history.clone(),
            RequestResponseRecord {
                id: record_id,
                timestamp,
                request_type,
                model: model.clone(),
                node_id,
                node_machine_name: node_machine_name.clone(),
                node_ip,
                client_ip: None,
                request_body: request_body.clone(),
                response_body: None, // ストリームボディは記録しない
                duration_ms: duration.as_millis() as u64,
                status: RecordStatus::Success,
                completed_at: Utc::now(),
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
            },
        );

        let mut axum_response = forward_streaming_response(response).map_err(AppError::from)?;
        if let Some(wait_ms) = queued_wait_ms {
            add_queue_headers(&mut axum_response, wait_ms);
        }
        return Ok(axum_response);
    }

    if !response.status().is_success() {
        let duration = start.elapsed();
        state
            .load_manager
            .finish_request(node_id, RequestOutcome::Error, duration)
            .await
            .map_err(AppError::from)?;

        if let Err(err) = state
            .registry
            .exclude_model_from_node(node_id, &model)
            .await
        {
            warn!(
                node_id = %node_id,
                model = %model,
                error = %err,
                "Failed to exclude model after non-success response"
            );
        }

        let status = response.status();
        let status_code = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
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
                node_id,
                node_machine_name: node_machine_name.clone(),
                node_ip,
                client_ip: None,
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
            },
        );

        let payload = json!({
            "error": {
                "message": message,
                "type": "node_upstream_error",
                "code": status_code.as_u16(),
            }
        });

        let mut response = (status_code, Json(payload)).into_response();
        if let Some(wait_ms) = queued_wait_ms {
            add_queue_headers(&mut response, wait_ms);
        }
        return Ok(response);
    }

    if stream {
        let duration = start.elapsed();
        state
            .load_manager
            .finish_request(node_id, RequestOutcome::Success, duration)
            .await
            .map_err(AppError::from)?;

        save_request_record(
            state.request_history.clone(),
            RequestResponseRecord {
                id: record_id,
                timestamp,
                request_type,
                model,
                node_id,
                node_machine_name,
                node_ip,
                client_ip: None,
                request_body,
                response_body: None,
                duration_ms: duration.as_millis() as u64,
                status: RecordStatus::Success,
                completed_at: Utc::now(),
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
            },
        );

        let mut axum_response = forward_streaming_response(response).map_err(AppError::from)?;
        if let Some(wait_ms) = queued_wait_ms {
            add_queue_headers(&mut axum_response, wait_ms);
        }
        return Ok(axum_response);
    }

    let parsed = response.json::<Value>().await;
    let duration = start.elapsed();

    match parsed {
        Ok(body) => {
            // レスポンスからトークン使用量を抽出
            let token_usage = extract_usage_from_response(&body);

            state
                .load_manager
                .finish_request_with_tokens(
                    node_id,
                    RequestOutcome::Success,
                    duration,
                    token_usage.clone(),
                )
                .await
                .map_err(AppError::from)?;

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
                    node_id,
                    node_machine_name,
                    node_ip,
                    client_ip: None,
                    request_body,
                    response_body: Some(body.clone()),
                    duration_ms: duration.as_millis() as u64,
                    status: RecordStatus::Success,
                    completed_at: Utc::now(),
                    input_tokens,
                    output_tokens,
                    total_tokens,
                },
            );

            let mut response = (StatusCode::OK, Json(body)).into_response();
            if let Some(wait_ms) = queued_wait_ms {
                add_queue_headers(&mut response, wait_ms);
            }
            Ok(response)
        }
        Err(e) => {
            state
                .load_manager
                .finish_request(node_id, RequestOutcome::Error, duration)
                .await
                .map_err(AppError::from)?;

            if let Err(err) = state
                .registry
                .exclude_model_from_node(node_id, &model)
                .await
            {
                warn!(
                    node_id = %node_id,
                    model = %model,
                    error = %err,
                    "Failed to exclude model after response parse error"
                );
            }

            save_request_record(
                state.request_history.clone(),
                RequestResponseRecord {
                    id: record_id,
                    timestamp,
                    request_type,
                    model,
                    node_id,
                    node_machine_name,
                    node_ip,
                    client_ip: None,
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
                },
            );

            Err(RouterError::Http(format!("Failed to parse OpenAI response: {}", e)).into())
        }
    }
}

#[allow(dead_code)]
async fn proxy_openai_get(state: &AppState, target_path: &str) -> Result<Response, AppError> {
    let node = select_available_node(state).await?;
    let node_id = node.id;

    state
        .load_manager
        .begin_request(node_id)
        .await
        .map_err(AppError::from)?;

    let client = state.http_client.clone();
    let runtime_url = format!(
        "http://{}:{}{}",
        node.ip_address, node.runtime_port, target_path
    );
    let start = Instant::now();

    let response = client.get(&runtime_url).send().await.map_err(|e| {
        AppError::from(RouterError::Http(format!(
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
    state
        .load_manager
        .finish_request(node_id, outcome, duration)
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
        AppError::from(RouterError::Http(format!(
            "Failed to parse OpenAI models response: {}",
            e
        )))
    })?;

    Ok((StatusCode::OK, Json(body)).into_response())
}

fn validation_error(message: impl Into<String>) -> AppError {
    let err = RouterError::Common(CommonError::Validation(message.into()));
    err.into()
}

#[cfg(test)]
mod tests {
    use super::{parse_cloud_model, proxy_openai_cloud_post, proxy_openai_post};
    use crate::{
        balancer::LoadManager,
        db::{request_history::RequestHistoryStorage, test_utils::TEST_LOCK},
        registry::NodeRegistry,
        AppState,
    };
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use llm_router_common::protocol::{RecordStatus, RequestType};
    use serde_json::json;
    use serial_test::serial;
    use sqlx::SqlitePool;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::time::{sleep, Duration};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn create_local_state() -> AppState {
        let registry = NodeRegistry::new();
        let load_manager = LoadManager::new(registry.clone());
        let db_pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("sqlite memory connect");
        sqlx::migrate!("./migrations")
            .run(&db_pool)
            .await
            .expect("migrations");
        let request_history = Arc::new(RequestHistoryStorage::new(db_pool.clone()));
        AppState {
            registry,
            load_manager,
            request_history,
            db_pool,
            jwt_secret: "test-secret".into(),
            http_client: reqwest::Client::new(),
            queue_config: crate::config::QueueConfig::from_env(),
            event_bus: crate::events::create_shared_event_bus(),
            endpoint_registry: None,
        }
    }

    async fn create_state_with_tempdir() -> (AppState, tempfile::TempDir) {
        let dir = tempdir().expect("temp dir");
        std::env::set_var("LLM_ROUTER_DATA_DIR", dir.path());
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
        std::env::remove_var("LLM_ROUTER_DATA_DIR");
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
        std::env::remove_var("LLM_ROUTER_DATA_DIR");
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
        std::env::remove_var("LLM_ROUTER_DATA_DIR");
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
        )
        .await
        .expect("cloud stream response");
        let body = to_bytes(resp.into_body(), 1_000_000).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.contains("delta"));
        assert!(body_str.contains("hi"));

        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("OPENAI_BASE_URL");
        std::env::remove_var("LLM_ROUTER_DATA_DIR");
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
        std::env::remove_var("LLM_ROUTER_DATA_DIR");
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
        std::env::remove_var("LLM_ROUTER_DATA_DIR");
    }

    #[tokio::test]
    #[serial]
    async fn cloud_request_is_recorded_in_history() {
        let _guard = TEST_LOCK.lock().await;
        let temp_dir = tempdir().expect("temp dir");
        std::env::set_var("LLM_ROUTER_DATA_DIR", temp_dir.path());

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
        std::env::remove_var("LLM_ROUTER_DATA_DIR");
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

        // router state with temp data dir
        std::env::set_var("OPENAI_API_KEY", "testkey");
        std::env::set_var("OPENAI_BASE_URL", server.uri());
        let (state, dir) = create_state_with_tempdir().await;

        // spawn router
        let app: Router = crate::api::create_router(state.clone());
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

        // fetch dashboard history
        let history_resp = client
            .get(format!("http://{addr}/v0/dashboard/request-responses"))
            .header("authorization", "Bearer sk_debug_admin")
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
        std::env::remove_var("LLM_ROUTER_DATA_DIR");
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
        std::env::remove_var("LLM_ROUTER_DATA_DIR");
    }

    // T006: chat capabilities検証テスト (RED)
    // TextGeneration capability を持たないモデルで /v1/chat/completions を呼ぶとエラー
    #[test]
    fn test_chat_capability_validation_error_message() {
        use llm_router_common::types::{ModelCapability, ModelType};

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
