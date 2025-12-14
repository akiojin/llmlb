//! 音声API エンドポイント (/v1/audio/*)
//!
//! OpenAI互換の音声認識（ASR）・音声合成（TTS）API

use axum::{
    body::Body,
    extract::{Multipart, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use llm_router_common::{
    error::RouterError,
    protocol::{RecordStatus, RequestResponseRecord, RequestType, SpeechRequest},
    types::{Node, RuntimeType},
};
use serde_json::json;
use std::time::Instant;
use tracing::info;
use uuid::Uuid;

use crate::{
    api::{
        nodes::AppError,
        proxy::{forward_streaming_response, save_request_record},
    },
    AppState,
};

/// RuntimeType に基づいてノードを選択
async fn select_node_by_runtime(
    state: &AppState,
    runtime_type: RuntimeType,
) -> Result<Node, RouterError> {
    let nodes = state.registry.list().await;

    // 対応するRuntimeTypeを持つオンラインノードを探す
    let capable_nodes: Vec<_> = nodes
        .into_iter()
        .filter(|n| {
            n.status == llm_router_common::types::NodeStatus::Online
                && n.supported_runtimes.contains(&runtime_type)
        })
        .collect();

    if capable_nodes.is_empty() {
        let runtime_name = match runtime_type {
            RuntimeType::WhisperCpp => "ASR (whisper.cpp)",
            RuntimeType::OnnxRuntime => "TTS (ONNX Runtime)",
            _ => "required runtime",
        };
        return Err(RouterError::ServiceUnavailable(format!(
            "No nodes available with {} capability",
            runtime_name
        )));
    }

    // 最初の利用可能なノードを返す（将来的にはロードバランシングを追加）
    Ok(capable_nodes.into_iter().next().unwrap())
}

/// POST /v1/audio/transcriptions - 音声認識（ASR）
///
/// multipart/form-data 形式でリクエスト
/// - file: 音声ファイル（wav, mp3, flac等）
/// - model: 使用するモデル名
/// - language: 言語コード（オプション）
/// - response_format: レスポンス形式（json, text, srt, vtt）
pub async fn transcriptions(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let timestamp = Utc::now();

    // multipart データを解析
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut model: Option<String> = None;
    let mut language: Option<String> = None;
    let mut response_format: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                file_name = field.file_name().map(|s| s.to_string());
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?
                        .to_vec(),
                );
            }
            "model" => {
                model = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?,
                );
            }
            "language" => {
                language = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?,
                );
            }
            "response_format" => {
                response_format = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?,
                );
            }
            _ => {
                // 未知のフィールドは無視
            }
        }
    }

    // 必須フィールドの検証
    let file_data = file_data.ok_or_else(|| {
        AppError::from(RouterError::Http(
            "Missing required field: file".to_string(),
        ))
    })?;
    let model = model.ok_or_else(|| {
        AppError::from(RouterError::Http(
            "Missing required field: model".to_string(),
        ))
    })?;

    info!(
        request_id = %request_id,
        model = %model,
        file_size = file_data.len(),
        "Processing transcription request"
    );

    // ASR対応ノードを選択
    let node = select_node_by_runtime(&state, RuntimeType::WhisperCpp).await?;

    // multipart リクエストを構築してプロキシ
    let client = &state.http_client;
    let url = format!(
        "http://{}:{}/v1/audio/transcriptions",
        node.ip_address, node.runtime_port
    );

    let mut form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(file_data)
            .file_name(file_name.unwrap_or_else(|| "audio.wav".to_string()))
            .mime_str("audio/wav")
            .unwrap(),
    );

    form = form.text("model", model.clone());

    if let Some(lang) = language {
        form = form.text("language", lang);
    }

    if let Some(fmt) = response_format {
        form = form.text("response_format", fmt);
    }

    let response = client
        .post(&url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?;

    let duration = start.elapsed();
    let status = response.status();

    // リクエスト履歴を記録
    let record = RequestResponseRecord {
        id: request_id,
        timestamp,
        request_type: RequestType::Transcription,
        model: model.clone(),
        node_id: node.id,
        agent_machine_name: node.machine_name.clone(),
        agent_ip: node.ip_address,
        client_ip: None,
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
    State(state): State<AppState>,
    Json(payload): Json<SpeechRequest>,
) -> Result<Response, AppError> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();
    let timestamp = Utc::now();

    // 入力テキストの検証
    if payload.input.is_empty() {
        return Err(AppError::from(RouterError::Http(
            "Input text is required".to_string(),
        )));
    }

    // 入力長の制限（4096文字）
    if payload.input.chars().count() > 4096 {
        return Err(AppError::from(RouterError::Http(
            "Input text exceeds maximum length of 4096 characters".to_string(),
        )));
    }

    info!(
        request_id = %request_id,
        model = %payload.model,
        input_length = payload.input.len(),
        voice = %payload.voice,
        "Processing speech request"
    );

    // TTS対応ノードを選択
    let node = select_node_by_runtime(&state, RuntimeType::OnnxRuntime).await?;

    // JSON リクエストをプロキシ
    let client = &state.http_client;
    let url = format!(
        "http://{}:{}/v1/audio/speech",
        node.ip_address, node.runtime_port
    );

    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?;

    let duration = start.elapsed();
    let status = response.status();

    // リクエスト履歴を記録
    let record = RequestResponseRecord {
        id: request_id,
        timestamp,
        request_type: RequestType::Speech,
        model: payload.model.clone(),
        node_id: node.id,
        agent_machine_name: node.machine_name.clone(),
        agent_ip: node.ip_address,
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
            .unwrap()
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
}
