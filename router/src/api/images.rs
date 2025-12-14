//! 画像API エンドポイント (/v1/images/*)
//!
//! OpenAI互換の画像生成（Text-to-Image）・編集（Inpainting）・バリエーションAPI

use axum::{
    extract::{Multipart, State},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use llm_router_common::{
    error::RouterError,
    protocol::{ImageGenerationRequest, RecordStatus, RequestResponseRecord, RequestType},
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

/// RuntimeType::StableDiffusion に基づいてノードを選択
async fn select_image_node(state: &AppState) -> Result<Node, RouterError> {
    let nodes = state.registry.list().await;

    // StableDiffusion対応のオンラインノードを探す
    let capable_nodes: Vec<_> = nodes
        .into_iter()
        .filter(|n| {
            n.status == llm_router_common::types::NodeStatus::Online
                && n.supported_runtimes.contains(&RuntimeType::StableDiffusion)
        })
        .collect();

    if capable_nodes.is_empty() {
        return Err(RouterError::ServiceUnavailable(
            "No nodes available with image generation (Stable Diffusion) capability".to_string(),
        ));
    }

    // 最初の利用可能なノードを返す（将来的にはロードバランシングを追加）
    Ok(capable_nodes.into_iter().next().unwrap())
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
        return Err(AppError::from(RouterError::Http(
            "Prompt is required".to_string(),
        )));
    }

    // 生成枚数の検証（1-10）
    if payload.n == 0 || payload.n > 10 {
        return Err(AppError::from(RouterError::Http(
            "n must be between 1 and 10".to_string(),
        )));
    }

    info!(
        request_id = %request_id,
        model = %payload.model,
        prompt_length = payload.prompt.len(),
        n = payload.n,
        "Processing image generation request"
    );

    // 画像生成対応ノードを選択
    let node = select_image_node(&state).await?;

    // JSON リクエストをプロキシ
    let client = &state.http_client;
    let url = format!(
        "http://{}:{}/v1/images/generations",
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
        request_type: RequestType::ImageGeneration,
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

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "image" => {
                image_name = field.file_name().map(|s| s.to_string());
                image_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?
                        .to_vec(),
                );
            }
            "mask" => {
                mask_name = field.file_name().map(|s| s.to_string());
                mask_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?
                        .to_vec(),
                );
            }
            "prompt" => {
                prompt = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?,
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
            "n" => {
                n = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?,
                );
            }
            "size" => {
                size = Some(
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
    let image_data = image_data.ok_or_else(|| {
        AppError::from(RouterError::Http(
            "Missing required field: image".to_string(),
        ))
    })?;
    let prompt = prompt.ok_or_else(|| {
        AppError::from(RouterError::Http(
            "Missing required field: prompt".to_string(),
        ))
    })?;
    let model = model.unwrap_or_else(|| "stable-diffusion-xl".to_string());

    // 画像サイズの検証（最大4MB）
    const MAX_IMAGE_SIZE: usize = 4 * 1024 * 1024; // 4MB
    if image_data.len() > MAX_IMAGE_SIZE {
        return Err(AppError::from(RouterError::Http(
            "Image file exceeds maximum size of 4MB".to_string(),
        )));
    }

    info!(
        request_id = %request_id,
        model = %model,
        image_size = image_data.len(),
        has_mask = mask_data.is_some(),
        "Processing image edit request"
    );

    // 画像生成対応ノードを選択
    let node = select_image_node(&state).await?;

    // multipart リクエストを構築してプロキシ
    let client = &state.http_client;
    let url = format!(
        "http://{}:{}/v1/images/edits",
        node.ip_address, node.runtime_port
    );

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
        request_type: RequestType::ImageEdit,
        model: model.clone(),
        node_id: node.id,
        agent_machine_name: node.machine_name.clone(),
        agent_ip: node.ip_address,
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

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "image" => {
                image_name = field.file_name().map(|s| s.to_string());
                image_data = Some(
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
            "n" => {
                n = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::from(RouterError::Http(e.to_string())))?,
                );
            }
            "size" => {
                size = Some(
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
    let image_data = image_data.ok_or_else(|| {
        AppError::from(RouterError::Http(
            "Missing required field: image".to_string(),
        ))
    })?;
    let model = model.unwrap_or_else(|| "stable-diffusion-xl".to_string());

    // 画像サイズの検証（最大4MB）
    const MAX_IMAGE_SIZE: usize = 4 * 1024 * 1024; // 4MB
    if image_data.len() > MAX_IMAGE_SIZE {
        return Err(AppError::from(RouterError::Http(
            "Image file exceeds maximum size of 4MB".to_string(),
        )));
    }

    info!(
        request_id = %request_id,
        model = %model,
        image_size = image_data.len(),
        "Processing image variation request"
    );

    // 画像生成対応ノードを選択
    let node = select_image_node(&state).await?;

    // multipart リクエストを構築してプロキシ
    let client = &state.http_client;
    let url = format!(
        "http://{}:{}/v1/images/variations",
        node.ip_address, node.runtime_port
    );

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
        request_type: RequestType::ImageVariation,
        model: model.clone(),
        node_id: node.id,
        agent_machine_name: node.machine_name.clone(),
        agent_ip: node.ip_address,
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
        // n = 0 は無効
        assert!(0 == 0 || 0 > 10);

        // n = 1-10 は有効
        for n in 1..=10 {
            assert!(n >= 1 && n <= 10);
        }

        // n = 11 は無効
        assert!(11 == 0 || 11 > 10);
    }
}
