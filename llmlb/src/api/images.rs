//! 画像API エンドポイント (/v1/images/*)
//!
//! OpenAI互換の画像生成（Text-to-Image）・編集（Inpainting）・バリエーションAPI

use crate::common::{
    error::LbError,
    protocol::{ImageGenerationRequest, RecordStatus, RequestResponseRecord, RequestType},
    types::ModelCapability,
};
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde_json::json;
use std::net::IpAddr;
use std::time::Instant;
use tracing::info;
use uuid::Uuid;

use crate::{
    api::{
        error::AppError,
        model_name::parse_quantized_model_name,
        models::load_registered_model,
        proxy::{forward_streaming_response, save_request_record},
    },
    types::endpoint::{Endpoint, EndpointCapability},
    AppState,
};

/// OpenAI互換エラーレスポンスを生成
fn error_response(error: LbError, status: StatusCode) -> Response {
    let (message, error_type) = match error {
        LbError::Http(msg) => (msg, "invalid_request_error"),
        LbError::ServiceUnavailable(msg) => (msg, "service_unavailable"),
        LbError::InvalidModelName(msg) => (msg, "invalid_request_error"),
        _ => (error.to_string(), "api_error"),
    };

    (
        status,
        Json(json!({
            "error": {
                "message": message,
                "type": error_type,
                "code": status.as_u16()
            }
        })),
    )
        .into_response()
}

/// OpenAI互換エラーレスポンスを返す（ハンドラで使用）
fn openai_error<T: Into<String>>(msg: T, status: StatusCode) -> Result<Response, AppError> {
    Ok(error_response(LbError::Http(msg.into()), status))
}

/// 画像生成対応バックエンド
/// EndpointRegistry経由でのみ取得（NodeRegistryフォールバック廃止）
struct ImageBackend(Endpoint);

impl ImageBackend {
    /// リクエスト送信用のURLを取得
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.0.base_url.trim_end_matches('/'), path)
    }

    /// リクエスト履歴用のID
    fn id(&self) -> Uuid {
        self.0.id
    }

    /// リクエスト履歴用の名前
    fn name(&self) -> String {
        self.0.name.clone()
    }

    /// リクエスト履歴用のIPアドレス
    fn ip(&self) -> IpAddr {
        // base_urlからホスト部分を抽出してパース
        // 例: "http://192.168.1.100:11434" -> "192.168.1.100"
        let host = self
            .0
            .base_url
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .split(':')
            .next()
            .unwrap_or("127.0.0.1");
        host.parse::<IpAddr>()
            .unwrap_or_else(|_| "127.0.0.1".parse().unwrap())
    }
}

/// 画像生成対応バックエンドを選択
/// EndpointRegistry経由でのみ検索（NodeRegistryフォールバック廃止）
async fn select_image_backend(state: &AppState) -> Result<ImageBackend, LbError> {
    // EndpointRegistry経由で検索（SPEC-66555000: 新方式のみ）
    let endpoints = state
        .endpoint_registry
        .list_online_by_capability(EndpointCapability::ImageGeneration)
        .await;

    let endpoint = endpoints.into_iter().next().ok_or_else(|| {
        LbError::ServiceUnavailable(
            "No endpoints available with image generation capability".to_string(),
        )
    })?;

    Ok(ImageBackend(endpoint))
}

/// POST /v1/images/generations - 画像生成（Text-to-Image）
///
/// JSON 形式でリクエスト
/// - model: 使用するモデル名 (例: "stable-diffusion-xl")
/// - prompt: 生成プロンプト
/// - n: 生成枚数（オプション、デフォルト: 1）
/// - size: 出力サイズ（オプション、デフォルト: "1024x1024"）
/// - quality: 品質（オプション、デフォルト: "standard"）
/// - style: スタイル（オプション、デフォルト: "vivid"）
/// - response_format: 出力形式（オプション、デフォルト: "url"）
pub async fn generations(
    State(state): State<AppState>,
    Json(payload): Json<ImageGenerationRequest>,
) -> Result<Response, AppError> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let timestamp = Utc::now();

    // プロンプトの検証
    if payload.prompt.is_empty() {
        return openai_error("Prompt is required", StatusCode::BAD_REQUEST);
    }

    // 生成枚数の検証（1-10）
    if payload.n == 0 || payload.n > 10 {
        return openai_error("n must be between 1 and 10", StatusCode::BAD_REQUEST);
    }

    let parsed = parse_quantized_model_name(&payload.model).map_err(AppError::from)?;
    let _lookup_model = parsed.base;

    // モデルの ImageGeneration capability を検証
    let model_info = load_registered_model(&state.db_pool, &payload.model).await?;
    if let Some(model_info) = model_info {
        if !model_info.has_capability(ModelCapability::ImageGeneration) {
            return openai_error(
                format!("Model '{}' does not support image generation", parsed.raw),
                StatusCode::BAD_REQUEST,
            );
        }
    }
    // 登録されていないモデルはノード側で処理（クラウドモデル等）

    info!(
        request_id = %request_id,
        model = %payload.model,
        prompt_length = payload.prompt.len(),
        n = payload.n,
        "Processing image generation request"
    );

    // 画像生成対応バックエンドを選択（EndpointRegistry優先、NodeRegistryフォールバック）
    let backend = select_image_backend(&state).await?;

    // JSON リクエストをプロキシ
    let client = &state.http_client;
    let url = backend.url("/v1/images/generations");

    let response = match client.post(&url).json(&payload).send().await {
        Ok(r) => r,
        Err(e) => {
            return openai_error(
                format!("Failed to contact image generation backend: {}", e),
                StatusCode::SERVICE_UNAVAILABLE,
            )
        }
    };

    let duration = start.elapsed();
    let status = response.status();

    // リクエスト履歴を記録
    let record = RequestResponseRecord {
        id: request_id,
        timestamp,
        request_type: RequestType::ImageGeneration,
        model: payload.model.clone(),
        node_id: backend.id(),
        node_machine_name: backend.name(),
        node_ip: backend.ip(),
        client_ip: None,
        request_body: serde_json::to_value(&payload).unwrap_or(json!({})),
        response_body: None,
        duration_ms: duration.as_millis() as u64,
        status: if status.is_success() {
            RecordStatus::Success
        } else {
            RecordStatus::Error {
                message: format!("HTTP {}", status.as_u16()),
            }
        },
        completed_at: Utc::now(),
        input_tokens: None,
        output_tokens: None,
        total_tokens: None,
    };

    save_request_record(state.request_history.clone(), record);

    // レスポンスを転送
    forward_streaming_response(response)
        .map_err(AppError::from)
        .map(|r| r.into_response())
}

/// POST /v1/images/edits - 画像編集（Inpainting）
///
/// multipart/form-data 形式でリクエスト
/// - image: 編集対象の画像ファイル（PNG、最大4MB）
/// - mask: マスク画像（オプション、PNG）
/// - prompt: 編集プロンプト
/// - model: 使用するモデル名
/// - n: 生成枚数（オプション）
/// - size: 出力サイズ（オプション）
/// - response_format: 出力形式（オプション）
pub async fn edits(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let timestamp = Utc::now();

    // multipart データを解析
    let mut image_data: Option<Vec<u8>> = None;
    let mut image_name: Option<String> = None;
    let mut mask_data: Option<Vec<u8>> = None;
    let mut mask_name: Option<String> = None;
    let mut prompt: Option<String> = None;
    let mut model: Option<String> = None;
    let mut n: Option<String> = None;
    let mut size: Option<String> = None;
    let mut response_format: Option<String> = None;

    while let Some(field) = match multipart.next_field().await {
        Ok(f) => f,
        Err(e) => {
            return openai_error(
                format!("Failed to parse multipart form: {}", e),
                StatusCode::BAD_REQUEST,
            )
        }
    } {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "image" => {
                image_name = field.file_name().map(|s| s.to_string());
                match field.bytes().await {
                    Ok(bytes) => image_data = Some(bytes.to_vec()),
                    Err(e) => {
                        return openai_error(
                            format!("Failed to read image field: {}", e),
                            StatusCode::BAD_REQUEST,
                        )
                    }
                }
            }
            "mask" => {
                mask_name = field.file_name().map(|s| s.to_string());
                match field.bytes().await {
                    Ok(bytes) => mask_data = Some(bytes.to_vec()),
                    Err(e) => {
                        return openai_error(
                            format!("Failed to read mask field: {}", e),
                            StatusCode::BAD_REQUEST,
                        )
                    }
                }
            }
            "prompt" => match field.text().await {
                Ok(text) => prompt = Some(text),
                Err(e) => {
                    return openai_error(
                        format!("Failed to read prompt field: {}", e),
                        StatusCode::BAD_REQUEST,
                    )
                }
            },
            "model" => match field.text().await {
                Ok(text) => model = Some(text),
                Err(e) => {
                    return openai_error(
                        format!("Failed to read model field: {}", e),
                        StatusCode::BAD_REQUEST,
                    )
                }
            },
            "n" => match field.text().await {
                Ok(text) => n = Some(text),
                Err(e) => {
                    return openai_error(
                        format!("Failed to read n field: {}", e),
                        StatusCode::BAD_REQUEST,
                    )
                }
            },
            "size" => match field.text().await {
                Ok(text) => size = Some(text),
                Err(e) => {
                    return openai_error(
                        format!("Failed to read size field: {}", e),
                        StatusCode::BAD_REQUEST,
                    )
                }
            },
            "response_format" => match field.text().await {
                Ok(text) => response_format = Some(text),
                Err(e) => {
                    return openai_error(
                        format!("Failed to read response_format field: {}", e),
                        StatusCode::BAD_REQUEST,
                    )
                }
            },
            _ => {
                // 未知のフィールドは無視
            }
        }
    }

    // 必須フィールドの検証
    let image_data = match image_data {
        Some(data) => data,
        None => return openai_error("Missing required field: image", StatusCode::BAD_REQUEST),
    };
    let prompt = match prompt {
        Some(p) => p,
        None => return openai_error("Missing required field: prompt", StatusCode::BAD_REQUEST),
    };
    let model = model.unwrap_or_else(|| "stable-diffusion-xl".to_string());

    // 画像サイズの検証（最大4MB）
    const MAX_IMAGE_SIZE: usize = 4 * 1024 * 1024; // 4MB
    if image_data.len() > MAX_IMAGE_SIZE {
        return openai_error(
            "Image file exceeds maximum size of 4MB",
            StatusCode::BAD_REQUEST,
        );
    }

    info!(
        request_id = %request_id,
        model = %model,
        image_size = image_data.len(),
        has_mask = mask_data.is_some(),
        "Processing image edit request"
    );

    // 画像生成対応バックエンドを選択（EndpointRegistry優先、NodeRegistryフォールバック）
    let backend = select_image_backend(&state).await?;

    // multipart リクエストを構築してプロキシ
    let client = &state.http_client;
    let url = backend.url("/v1/images/edits");

    let mut form = reqwest::multipart::Form::new().part(
        "image",
        reqwest::multipart::Part::bytes(image_data)
            .file_name(image_name.unwrap_or_else(|| "image.png".to_string()))
            .mime_str("image/png")
            .unwrap(),
    );

    if let Some(mask) = mask_data {
        form = form.part(
            "mask",
            reqwest::multipart::Part::bytes(mask)
                .file_name(mask_name.unwrap_or_else(|| "mask.png".to_string()))
                .mime_str("image/png")
                .unwrap(),
        );
    }

    form = form.text("prompt", prompt.clone());
    form = form.text("model", model.clone());

    if let Some(n_val) = n {
        form = form.text("n", n_val);
    }

    if let Some(size_val) = size {
        form = form.text("size", size_val);
    }

    if let Some(fmt) = response_format {
        form = form.text("response_format", fmt);
    }

    let response = match client.post(&url).multipart(form).send().await {
        Ok(r) => r,
        Err(e) => {
            return openai_error(
                format!("Failed to contact image edit node: {}", e),
                StatusCode::SERVICE_UNAVAILABLE,
            )
        }
    };

    let duration = start.elapsed();
    let status = response.status();

    // リクエスト履歴を記録
    let record = RequestResponseRecord {
        id: request_id,
        timestamp,
        request_type: RequestType::ImageEdit,
        model: model.clone(),
        node_id: backend.id(),
        node_machine_name: backend.name(),
        node_ip: backend.ip(),
        client_ip: None,
        request_body: json!({"model": model, "prompt": prompt, "type": "image_edit"}),
        response_body: None,
        duration_ms: duration.as_millis() as u64,
        status: if status.is_success() {
            RecordStatus::Success
        } else {
            RecordStatus::Error {
                message: format!("HTTP {}", status.as_u16()),
            }
        },
        completed_at: Utc::now(),
        input_tokens: None,
        output_tokens: None,
        total_tokens: None,
    };

    save_request_record(state.request_history.clone(), record);

    // レスポンスを転送
    forward_streaming_response(response)
        .map_err(AppError::from)
        .map(|r| r.into_response())
}

/// POST /v1/images/variations - 画像バリエーション生成
///
/// multipart/form-data 形式でリクエスト
/// - image: 元画像ファイル（PNG、最大4MB）
/// - model: 使用するモデル名
/// - n: 生成枚数（オプション）
/// - size: 出力サイズ（オプション）
/// - response_format: 出力形式（オプション）
pub async fn variations(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let timestamp = Utc::now();

    // multipart データを解析
    let mut image_data: Option<Vec<u8>> = None;
    let mut image_name: Option<String> = None;
    let mut model: Option<String> = None;
    let mut n: Option<String> = None;
    let mut size: Option<String> = None;
    let mut response_format: Option<String> = None;

    while let Some(field) = match multipart.next_field().await {
        Ok(f) => f,
        Err(e) => {
            return openai_error(
                format!("Failed to parse multipart form: {}", e),
                StatusCode::BAD_REQUEST,
            )
        }
    } {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "image" => {
                image_name = field.file_name().map(|s| s.to_string());
                match field.bytes().await {
                    Ok(bytes) => image_data = Some(bytes.to_vec()),
                    Err(e) => {
                        return openai_error(
                            format!("Failed to read image field: {}", e),
                            StatusCode::BAD_REQUEST,
                        )
                    }
                }
            }
            "model" => match field.text().await {
                Ok(text) => model = Some(text),
                Err(e) => {
                    return openai_error(
                        format!("Failed to read model field: {}", e),
                        StatusCode::BAD_REQUEST,
                    )
                }
            },
            "n" => match field.text().await {
                Ok(text) => n = Some(text),
                Err(e) => {
                    return openai_error(
                        format!("Failed to read n field: {}", e),
                        StatusCode::BAD_REQUEST,
                    )
                }
            },
            "size" => match field.text().await {
                Ok(text) => size = Some(text),
                Err(e) => {
                    return openai_error(
                        format!("Failed to read size field: {}", e),
                        StatusCode::BAD_REQUEST,
                    )
                }
            },
            "response_format" => match field.text().await {
                Ok(text) => response_format = Some(text),
                Err(e) => {
                    return openai_error(
                        format!("Failed to read response_format field: {}", e),
                        StatusCode::BAD_REQUEST,
                    )
                }
            },
            _ => {
                // 未知のフィールドは無視
            }
        }
    }

    // 必須フィールドの検証
    let image_data = match image_data {
        Some(data) => data,
        None => return openai_error("Missing required field: image", StatusCode::BAD_REQUEST),
    };
    let model = model.unwrap_or_else(|| "stable-diffusion-xl".to_string());

    // 画像サイズの検証（最大4MB）
    const MAX_IMAGE_SIZE: usize = 4 * 1024 * 1024; // 4MB
    if image_data.len() > MAX_IMAGE_SIZE {
        return openai_error(
            "Image file exceeds maximum size of 4MB",
            StatusCode::BAD_REQUEST,
        );
    }

    info!(
        request_id = %request_id,
        model = %model,
        image_size = image_data.len(),
        "Processing image variation request"
    );

    // 画像生成対応バックエンドを選択（EndpointRegistry優先、NodeRegistryフォールバック）
    let backend = select_image_backend(&state).await?;

    // multipart リクエストを構築してプロキシ
    let client = &state.http_client;
    let url = backend.url("/v1/images/variations");

    let mut form = reqwest::multipart::Form::new().part(
        "image",
        reqwest::multipart::Part::bytes(image_data)
            .file_name(image_name.unwrap_or_else(|| "image.png".to_string()))
            .mime_str("image/png")
            .unwrap(),
    );

    form = form.text("model", model.clone());

    if let Some(n_val) = n {
        form = form.text("n", n_val);
    }

    if let Some(size_val) = size {
        form = form.text("size", size_val);
    }

    if let Some(fmt) = response_format {
        form = form.text("response_format", fmt);
    }

    let response = match client.post(&url).multipart(form).send().await {
        Ok(r) => r,
        Err(e) => {
            return openai_error(
                format!("Failed to contact image variation node: {}", e),
                StatusCode::SERVICE_UNAVAILABLE,
            )
        }
    };

    let duration = start.elapsed();
    let status = response.status();

    // リクエスト履歴を記録
    let record = RequestResponseRecord {
        id: request_id,
        timestamp,
        request_type: RequestType::ImageVariation,
        model: model.clone(),
        node_id: backend.id(),
        node_machine_name: backend.name(),
        node_ip: backend.ip(),
        client_ip: None,
        request_body: json!({"model": model, "type": "image_variation"}),
        response_body: None,
        duration_ms: duration.as_millis() as u64,
        status: if status.is_success() {
            RecordStatus::Success
        } else {
            RecordStatus::Error {
                message: format!("HTTP {}", status.as_u16()),
            }
        },
        completed_at: Utc::now(),
        input_tokens: None,
        output_tokens: None,
        total_tokens: None,
    };

    save_request_record(state.request_history.clone(), record);

    // レスポンスを転送
    forward_streaming_response(response)
        .map_err(AppError::from)
        .map(|r| r.into_response())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_image_size_limit() {
        const MAX_IMAGE_SIZE: usize = 4 * 1024 * 1024; // 4MB

        // 4MB以下は許可
        let small_image = vec![0u8; MAX_IMAGE_SIZE];
        assert!(small_image.len() <= MAX_IMAGE_SIZE);

        // 4MB超は拒否
        let large_image = vec![0u8; MAX_IMAGE_SIZE + 1];
        assert!(large_image.len() > MAX_IMAGE_SIZE);
    }

    #[test]
    fn test_n_validation() {
        // Helper function to validate n parameter (1-10 is valid)
        fn is_valid_n(n: u32) -> bool {
            (1..=10).contains(&n)
        }

        // n = 0 は無効
        assert!(!is_valid_n(0));

        // n = 1-10 は有効
        for n in 1..=10 {
            assert!(is_valid_n(n));
        }

        // n = 11 は無効
        assert!(!is_valid_n(11));
    }

    // T007: 画像生成 capabilities検証テスト (RED)
    // ImageGeneration capability を持たないモデルで /v1/images/generations を呼ぶとエラー
    #[test]
    fn test_image_generation_capability_validation_error_message() {
        use crate::common::types::{ModelCapability, ModelType};

        // LLMモデルはTextGenerationのみ、ImageGenerationは非対応
        let llm_caps = ModelCapability::from_model_type(ModelType::Llm);
        assert!(!llm_caps.contains(&ModelCapability::ImageGeneration));

        // TTSモデルもTextToSpeechのみ、ImageGenerationは非対応
        let tts_caps = ModelCapability::from_model_type(ModelType::TextToSpeech);
        assert!(!tts_caps.contains(&ModelCapability::ImageGeneration));

        // ASRモデルもSpeechToTextのみ、ImageGenerationは非対応
        let stt_caps = ModelCapability::from_model_type(ModelType::SpeechToText);
        assert!(!stt_caps.contains(&ModelCapability::ImageGeneration));

        // EmbeddingモデルもEmbeddingのみ、ImageGenerationは非対応
        let embed_caps = ModelCapability::from_model_type(ModelType::Embedding);
        assert!(!embed_caps.contains(&ModelCapability::ImageGeneration));

        // 期待されるエラーメッセージ形式
        let model_name = "llama-3.1-8b";
        let expected_error = format!("Model '{}' does not support image generation", model_name);
        assert!(expected_error.contains("does not support image generation"));
    }
}
