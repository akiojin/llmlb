//! モデル同期モジュール
//!
//! エンドポイントからモデル一覧を取得し、DBと同期

pub mod capabilities;
pub mod parser;

pub use capabilities::{
    capabilities_to_strings, capability_from_str, detect_capabilities, Capability,
};
pub use parser::{parse_models_response, ParsedModel, ResponseFormat};

use crate::db::endpoints as db;
use crate::types::endpoint::{EndpointModel, SupportedAPI};
use chrono::Utc;
use reqwest::Client;
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::time::Duration;
use uuid::Uuid;

/// 同期結果
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// 同期されたモデル
    pub models: Vec<EndpointModel>,
    /// 追加されたモデル数
    pub added: usize,
    /// 削除されたモデル数
    pub removed: usize,
    /// 更新されたモデル数（既存モデルの再確認）
    pub updated: usize,
    /// 検出されたレスポンス形式
    pub format: ResponseFormat,
}

/// 同期エラー
#[derive(Debug)]
pub enum SyncError {
    /// HTTP接続エラー
    ConnectionError(String),
    /// HTTPエラーレスポンス
    HttpError(u16, String),
    /// パースエラー
    ParseError(String),
    /// DBエラー
    DbError(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            SyncError::HttpError(status, msg) => write!(f, "HTTP {}: {}", status, msg),
            SyncError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            SyncError::DbError(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl std::error::Error for SyncError {}

/// エンドポイントからモデル一覧を取得してDBと同期
///
/// # 処理フロー
/// 1. GET /v1/models でモデル一覧を取得
/// 2. OpenAI/Ollama形式をパース
/// 3. 既存モデルと比較（差分計算）
/// 4. DBを更新（削除→追加）
/// 5. capabilitiesを自動判定
pub async fn sync_models(
    pool: &SqlitePool,
    client: &Client,
    endpoint_id: Uuid,
    base_url: &str,
    api_key: Option<&str>,
    timeout_secs: u64,
) -> Result<SyncResult, SyncError> {
    // 既存モデルを取得
    let existing_models: HashSet<String> = match db::list_endpoint_models(pool, endpoint_id).await {
        Ok(models) => models.into_iter().map(|m| m.model_id).collect(),
        Err(_) => HashSet::new(),
    };

    // GET /v1/models でモデル一覧を取得
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));

    let mut request = client.get(&url);
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    let response = request
        .timeout(Duration::from_secs(timeout_secs))
        .send()
        .await
        .map_err(|e| SyncError::ConnectionError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(SyncError::HttpError(status, body));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| SyncError::ParseError(e.to_string()))?;

    // モデル一覧をパース
    let (parsed_models, format) = parse_models_response(&json);

    // 新しいモデルIDのセット
    let new_model_ids: HashSet<String> = parsed_models.iter().map(|m| m.id.clone()).collect();

    // 差分を計算
    let added_ids: Vec<_> = new_model_ids.difference(&existing_models).collect();
    let removed_ids: Vec<_> = existing_models.difference(&new_model_ids).collect();
    let updated_ids: Vec<_> = new_model_ids.intersection(&existing_models).collect();

    let added = added_ids.len();
    let removed = removed_ids.len();
    let updated = updated_ids.len();

    // 削除されたモデルを削除
    for model_id in removed_ids {
        let _ = db::delete_endpoint_model(pool, endpoint_id, model_id).await;
    }

    // 新しいモデルを追加（capabilitiesを自動判定 + エンドポイントからの情報を使用）
    let now = Utc::now();
    let mut synced_models = Vec::new();

    // Build a lookup map for has_vision from parsed models
    let vision_lookup: std::collections::HashMap<String, bool> = parsed_models
        .iter()
        .map(|m| (m.id.clone(), m.has_vision))
        .collect();

    for model_id in &added_ids {
        let mut caps = detect_capabilities(model_id);
        // If endpoint reports vision capability, add it
        if let Some(&has_vision) = vision_lookup.get(*model_id) {
            if has_vision && !caps.contains(&Capability::Vision) {
                caps.push(Capability::Vision);
            }
        }
        let caps_vec = Some(capabilities_to_strings(&caps));

        let model = EndpointModel {
            endpoint_id,
            model_id: (*model_id).clone(),
            capabilities: caps_vec,
            last_checked: Some(now),
            supported_apis: vec![SupportedAPI::ChatCompletions],
        };

        let _ = db::add_endpoint_model(pool, &model).await;
        synced_models.push(model);
    }

    // 既存モデルのlast_checkedを更新
    for model_id in &updated_ids {
        let mut caps = detect_capabilities(model_id);
        // If endpoint reports vision capability, add it
        if let Some(&has_vision) = vision_lookup.get(*model_id) {
            if has_vision && !caps.contains(&Capability::Vision) {
                caps.push(Capability::Vision);
            }
        }
        let caps_vec = Some(capabilities_to_strings(&caps));

        let model = EndpointModel {
            endpoint_id,
            model_id: (*model_id).clone(),
            capabilities: caps_vec,
            last_checked: Some(now),
            supported_apis: vec![SupportedAPI::ChatCompletions],
        };

        let _ = db::update_endpoint_model(pool, &model).await;
        synced_models.push(model);
    }

    Ok(SyncResult {
        models: synced_models,
        added,
        removed,
        updated,
        format,
    })
}

/// 2つのモデルセット間の差分を計算
///
/// # Returns
/// (追加されるモデル, 削除されるモデル, 更新されるモデル)
pub fn calculate_diff(
    existing: &HashSet<String>,
    new: &HashSet<String>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let added: Vec<String> = new.difference(existing).cloned().collect();
    let removed: Vec<String> = existing.difference(new).cloned().collect();
    let updated: Vec<String> = new.intersection(existing).cloned().collect();

    (added, removed, updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_diff_all_new() {
        let existing = HashSet::new();
        let new: HashSet<String> = ["a", "b", "c"].into_iter().map(String::from).collect();

        let (added, removed, updated) = calculate_diff(&existing, &new);
        assert_eq!(added.len(), 3);
        assert!(removed.is_empty());
        assert!(updated.is_empty());
    }

    #[test]
    fn test_calculate_diff_all_removed() {
        let existing: HashSet<String> = ["a", "b", "c"].into_iter().map(String::from).collect();
        let new = HashSet::new();

        let (added, removed, updated) = calculate_diff(&existing, &new);
        assert!(added.is_empty());
        assert_eq!(removed.len(), 3);
        assert!(updated.is_empty());
    }

    #[test]
    fn test_calculate_diff_mixed() {
        let existing: HashSet<String> = ["a", "b", "c"].into_iter().map(String::from).collect();
        let new: HashSet<String> = ["b", "c", "d"].into_iter().map(String::from).collect();

        let (added, removed, updated) = calculate_diff(&existing, &new);
        assert_eq!(added, vec!["d"]);
        assert_eq!(removed, vec!["a"]);
        assert_eq!(updated.len(), 2);
        assert!(updated.contains(&"b".to_string()));
        assert!(updated.contains(&"c".to_string()));
    }

    #[test]
    fn test_calculate_diff_no_change() {
        let existing: HashSet<String> = ["a", "b"].into_iter().map(String::from).collect();
        let new = existing.clone();

        let (added, removed, updated) = calculate_diff(&existing, &new);
        assert!(added.is_empty());
        assert!(removed.is_empty());
        assert_eq!(updated.len(), 2);
    }

    #[test]
    fn test_sync_error_display() {
        let err = SyncError::ConnectionError("timeout".to_string());
        assert!(err.to_string().contains("timeout"));

        let err = SyncError::HttpError(404, "Not found".to_string());
        assert!(err.to_string().contains("404"));

        let err = SyncError::ParseError("invalid json".to_string());
        assert!(err.to_string().contains("Parse error"));

        let err = SyncError::DbError("constraint violation".to_string());
        assert!(err.to_string().contains("Database error"));
    }
}
