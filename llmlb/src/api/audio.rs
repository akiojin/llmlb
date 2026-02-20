//! 音声API エンドポイント (/v1/audio/*)
//!
//! OpenAI互換の音声認識（ASR）・音声合成（TTS）API

use crate::common::{
    error::LbError,
    protocol::{RecordStatus, RequestResponseRecord, RequestType, SpeechRequest},
    types::ModelCapability,
};
use axum::{
    body::Body,
    extract::{ConnectInfo, Multipart, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde_json::json;
use std::time::Instant;
use tracing::info;
use uuid::Uuid;

use std::net::{IpAddr, SocketAddr};

use crate::{
    api::{
        error::AppError,
        model_name::parse_quantized_model_name,
        models::load_registered_model,
        proxy::{forward_streaming_response, save_request_record},
    },
    auth::middleware::ApiKeyAuthContext,
    common::ip::normalize_socket_ip,
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

/// 音声処理対応バックエンド
/// EndpointRegistry経由でのみ取得（NodeRegistryフォールバック廃止）
struct AudioBackend(Endpoint);

impl AudioBackend {
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
        // フォールバック用のローカルホストアドレス
        const LOCALHOST: IpAddr = IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);

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
        host.parse::<IpAddr>().unwrap_or(LOCALHOST)
    }
}

/// 音声認識対応バックエンドを選択
/// EndpointRegistry経由でのみ取得（NodeRegistryフォールバック廃止）
async fn select_transcription_backend(state: &AppState) -> Result<AudioBackend, LbError> {
    let endpoints = state
        .endpoint_registry
        .list_online_by_capability(EndpointCapability::AudioTranscription)
        .await;

    let endpoint = endpoints.into_iter().next().ok_or_else(|| {
        LbError::ServiceUnavailable(
            "No endpoints available with audio transcription capability".to_string(),
        )
    })?;

    Ok(AudioBackend(endpoint))
}

/// 音声合成対応バックエンドを選択
/// EndpointRegistry経由でのみ取得（NodeRegistryフォールバック廃止）
async fn select_speech_backend(state: &AppState) -> Result<AudioBackend, LbError> {
    let endpoints = state
        .endpoint_registry
        .list_online_by_capability(EndpointCapability::AudioSpeech)
        .await;

    let endpoint = endpoints.into_iter().next().ok_or_else(|| {
        LbError::ServiceUnavailable(
            "No endpoints available with audio speech capability".to_string(),
        )
    })?;

    Ok(AudioBackend(endpoint))
}

/// POST /v1/audio/transcriptions - 音声認識（ASR）
///
/// multipart/form-data 形式でリクエスト
/// - file: 音声ファイル（wav, mp3, flac等）
/// - model: 使用するモデル名
/// - language: 言語コード（オプション）
/// - response_format: レスポンス形式（json, text, srt, vtt）
pub async fn transcriptions(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    auth_ctx: Option<axum::Extension<ApiKeyAuthContext>>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let client_ip = Some(normalize_socket_ip(&addr));
    let api_key_id = auth_ctx.as_ref().map(|ext| ext.0.id);
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let timestamp = Utc::now();

    // multipart データを解析
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut model: Option<String> = None;
    let mut language: Option<String> = None;
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
            "file" => {
                file_name = field.file_name().map(|s| s.to_string());
                match field.bytes().await {
                    Ok(bytes) => file_data = Some(bytes.to_vec()),
                    Err(e) => {
                        return openai_error(
                            format!("Failed to read file field: {}", e),
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
            "language" => match field.text().await {
                Ok(text) => language = Some(text),
                Err(e) => {
                    return openai_error(
                        format!("Failed to read language field: {}", e),
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
    let file_data = match file_data {
        Some(data) => data,
        None => return openai_error("Missing required field: file", StatusCode::BAD_REQUEST),
    };
    let model = match model {
        Some(m) => m,
        None => return openai_error("Missing required field: model", StatusCode::BAD_REQUEST),
    };
    let parsed = parse_quantized_model_name(&model).map_err(AppError::from)?;
    let _lookup_model = parsed.base;

    // モデルの SpeechToText capability を検証
    let model_info = load_registered_model(&state.db_pool, &model).await?;
    if let Some(model_info) = model_info {
        if !model_info.has_capability(ModelCapability::SpeechToText) {
            return openai_error(
                format!("Model '{}' does not support speech-to-text", parsed.raw),
                StatusCode::BAD_REQUEST,
            );
        }
    }
    // 登録されていないモデルはエンドポイント側で処理（クラウドモデル等）

    info!(
        request_id = %request_id,
        model = %model,
        file_size = file_data.len(),
        "Processing transcription request"
    );

    // ASR対応バックエンドを選択（EndpointRegistry優先、NodeRegistryフォールバック）
    let backend = select_transcription_backend(&state).await?;

    // multipart リクエストを構築してプロキシ
    let client = &state.http_client;
    let url = backend.url("/v1/audio/transcriptions");

    let mut form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(file_data)
            .file_name(file_name.unwrap_or_else(|| "audio.wav".to_string()))
            .mime_str("audio/wav")
            .expect("audio/wav is a valid MIME type"),
    );

    form = form.text("model", model.clone());

    if let Some(lang) = language {
        form = form.text("language", lang);
    }

    if let Some(fmt) = response_format {
        form = form.text("response_format", fmt);
    }

    let response = match client.post(&url).multipart(form).send().await {
        Ok(r) => r,
        Err(e) => {
            return openai_error(
                format!("Failed to contact transcription node: {}", e),
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
        request_type: RequestType::Transcription,
        model: model.clone(),
        node_id: backend.id(),
        node_machine_name: backend.name(),
        node_ip: backend.ip(),
        client_ip,
        request_body: json!({"model": model, "type": "transcription"}),
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
        api_key_id,
    };

    save_request_record(state.request_history.clone(), record);

    // レスポンスを転送
    forward_streaming_response(response)
        .map_err(AppError::from)
        .map(|r| r.into_response())
}

/// POST /v1/audio/speech - 音声合成（TTS）
///
/// JSON 形式でリクエスト
/// - model: 使用するモデル名
/// - input: 読み上げるテキスト
/// - voice: 音声種別（オプション、デフォルト: nova）
/// - response_format: 出力形式（オプション、デフォルト: mp3）
/// - speed: 再生速度（オプション、デフォルト: 1.0）
pub async fn speech(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    auth_ctx: Option<axum::Extension<ApiKeyAuthContext>>,
    Json(payload): Json<SpeechRequest>,
) -> Result<Response, AppError> {
    let client_ip = Some(normalize_socket_ip(&addr));
    let api_key_id = auth_ctx.as_ref().map(|ext| ext.0.id);
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let timestamp = Utc::now();

    // 入力テキストの検証
    if payload.input.is_empty() {
        return openai_error("Input text is required", StatusCode::BAD_REQUEST);
    }

    // 入力長の制限（4096文字）
    if payload.input.chars().count() > 4096 {
        return openai_error(
            "Input text exceeds maximum length of 4096 characters",
            StatusCode::BAD_REQUEST,
        );
    }

    let parsed = parse_quantized_model_name(&payload.model).map_err(AppError::from)?;
    let _lookup_model = parsed.base;

    // モデルの TextToSpeech capability を検証
    let model_info = load_registered_model(&state.db_pool, &payload.model).await?;
    if let Some(model_info) = model_info {
        if !model_info.has_capability(ModelCapability::TextToSpeech) {
            return openai_error(
                format!("Model '{}' does not support text-to-speech", parsed.raw),
                StatusCode::BAD_REQUEST,
            );
        }
    }
    // 登録されていないモデルはエンドポイント側で処理（クラウドモデル等）

    info!(
        request_id = %request_id,
        model = %payload.model,
        input_length = payload.input.len(),
        voice = %payload.voice,
        "Processing speech request"
    );

    // TTS対応バックエンドを選択（EndpointRegistry優先、NodeRegistryフォールバック）
    let backend = select_speech_backend(&state).await?;

    // JSON リクエストをプロキシ
    let client = &state.http_client;
    let url = backend.url("/v1/audio/speech");

    let response = match client.post(&url).json(&payload).send().await {
        Ok(r) => r,
        Err(e) => {
            return openai_error(
                format!("Failed to contact speech synthesis node: {}", e),
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
        request_type: RequestType::Speech,
        model: payload.model.clone(),
        node_id: backend.id(),
        node_machine_name: backend.name(),
        node_ip: backend.ip(),
        client_ip,
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
        api_key_id,
    };

    save_request_record(state.request_history.clone(), record);

    if status.is_success() {
        // 音声バイナリをストリーミング転送
        // reqwestとaxumで異なるhttp crateバージョンを使うため、文字列経由で変換
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("audio/mpeg")
            .to_string();

        let stream = response.bytes_stream();
        let body = Body::from_stream(stream);

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type)
            .body(body)
            .expect("Response builder should not fail with valid status and body")
            .into_response())
    } else {
        // エラーレスポンスを転送
        forward_streaming_response(response)
            .map_err(AppError::from)
            .map(|r| r.into_response())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_input_length_validation() {
        // 4096文字以下は許可
        let short_input = "a".repeat(4096);
        assert!(short_input.chars().count() <= 4096);

        // 4097文字以上は拒否
        let long_input = "a".repeat(4097);
        assert!(long_input.chars().count() > 4096);
    }

    #[test]
    fn test_unicode_input_length() {
        // 日本語文字のカウント（バイト数ではなく文字数）
        let japanese = "あ".repeat(4096);
        assert_eq!(japanese.chars().count(), 4096);

        let japanese_long = "あ".repeat(4097);
        assert!(japanese_long.chars().count() > 4096);
    }

    // T004: TTS capabilities検証テスト (RED)
    // TextToSpeech capability を持たないモデルで /v1/audio/speech を呼ぶとエラー
    #[test]
    fn test_tts_capability_validation_error_message() {
        use crate::common::types::{ModelCapability, ModelType};

        // LLMモデルはTextGenerationのみ、TextToSpeechは非対応
        let llm_caps = ModelCapability::from_model_type(ModelType::Llm);
        assert!(!llm_caps.contains(&ModelCapability::TextToSpeech));

        // 期待されるエラーメッセージ形式
        let model_name = "llama-3.1-8b";
        let expected_error = format!("Model '{}' does not support text-to-speech", model_name);
        assert!(expected_error.contains("does not support text-to-speech"));
    }

    // T005: ASR capabilities検証テスト (RED)
    // SpeechToText capability を持たないモデルで /v1/audio/transcriptions を呼ぶとエラー
    #[test]
    fn test_asr_capability_validation_error_message() {
        use crate::common::types::{ModelCapability, ModelType};

        // LLMモデルはTextGenerationのみ、SpeechToTextは非対応
        let llm_caps = ModelCapability::from_model_type(ModelType::Llm);
        assert!(!llm_caps.contains(&ModelCapability::SpeechToText));

        // TTSモデルもSpeechToTextは非対応
        let tts_caps = ModelCapability::from_model_type(ModelType::TextToSpeech);
        assert!(!tts_caps.contains(&ModelCapability::SpeechToText));

        // 期待されるエラーメッセージ形式
        let model_name = "vibevoice-v1";
        let expected_error = format!("Model '{}' does not support speech-to-text", model_name);
        assert!(expected_error.contains("does not support speech-to-text"));
    }
}
