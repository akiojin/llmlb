//! モデル管理API
//!
//! モデル一覧取得エンドポイント
//!
//! NOTE: SPEC-93536000 により、ルーター側のモデルレジストリ（REGISTERED_MODELS）は廃止されました。
//! モデル一覧は /v1/models（ノードの executable_models 集約）を使用してください。
//! ダッシュボードの Model Hub 機能はスコープ外です。

use axum::{http::StatusCode, response::IntoResponse, Json};
use llm_router_common::error::RouterError;
use serde::{Deserialize, Serialize};

/// モデルのライフサイクル状態
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleStatus {
    /// 登録リクエスト受付、キャッシュ待ち
    Pending,
    /// ダウンロード・変換中（キャッシュ処理中）
    Caching,
    /// ルーターにキャッシュ完了（ノードがアクセス可能）
    Registered,
    /// エラー発生
    Error,
}

/// モデルの状態（SPEC-6cd7f960）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelStatus {
    /// 対応モデル（未ダウンロード）
    Available,
    /// ダウンロード中
    Downloading,
    /// ダウンロード完了
    Downloaded,
}

/// ダウンロード進行状況
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    /// 進行率（0.0〜1.0）
    pub percent: f64,
    /// ダウンロード済みバイト数
    pub bytes_downloaded: Option<u64>,
    /// 総バイト数
    pub bytes_total: Option<u64>,
    /// エラーメッセージ（status=errorの場合）
    pub error: Option<String>,
}

/// 登録モデルのインメモリキャッシュをクリア（テスト用）
///
/// NOTE: SPEC-93536000 で REGISTERED_MODELS を廃止したため、このAPIは空実装です。
/// 既存テストとの互換性のために関数シグネチャを維持しています。
pub fn clear_registered_models() {
    // No-op: REGISTERED_MODELS は廃止されました
}

/// Axum用のエラーレスポンス型
#[derive(Debug)]
pub struct AppError(RouterError);

impl From<RouterError> for AppError {
    fn from(err: RouterError) -> Self {
        AppError(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self.0 {
            RouterError::NodeNotFound(_) => (StatusCode::NOT_FOUND, self.0.to_string()),
            RouterError::NoNodesAvailable => (StatusCode::SERVICE_UNAVAILABLE, self.0.to_string()),
            RouterError::NoCapableNodes(_) => (StatusCode::SERVICE_UNAVAILABLE, self.0.to_string()),
            RouterError::ModelNotFound(_) => (StatusCode::NOT_FOUND, self.0.to_string()),
            RouterError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.clone()),
            RouterError::NodeOffline(_) => (StatusCode::SERVICE_UNAVAILABLE, self.0.to_string()),
            RouterError::InvalidModelName(_) => (StatusCode::BAD_REQUEST, self.0.to_string()),
            RouterError::InsufficientStorage(_) => {
                (StatusCode::INSUFFICIENT_STORAGE, self.0.to_string())
            }
            RouterError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
            RouterError::Http(_) => (StatusCode::BAD_GATEWAY, self.0.to_string()),
            RouterError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, self.0.to_string()),
            RouterError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
            RouterError::PasswordHash(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
            RouterError::Jwt(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
            RouterError::Authentication(_) => (StatusCode::UNAUTHORIZED, self.0.to_string()),
            RouterError::Authorization(_) => (StatusCode::FORBIDDEN, self.0.to_string()),
            RouterError::Common(err) => (StatusCode::BAD_REQUEST, err.to_string()),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_status_serialization() {
        // ModelStatusが正しくシリアライズされることを確認
        assert_eq!(
            serde_json::to_string(&ModelStatus::Available).unwrap(),
            "\"available\""
        );
        assert_eq!(
            serde_json::to_string(&ModelStatus::Downloading).unwrap(),
            "\"downloading\""
        );
        assert_eq!(
            serde_json::to_string(&ModelStatus::Downloaded).unwrap(),
            "\"downloaded\""
        );
    }

    #[test]
    fn test_lifecycle_status_serialization() {
        // LifecycleStatusが正しくシリアライズされることを確認
        assert_eq!(
            serde_json::to_string(&LifecycleStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&LifecycleStatus::Registered).unwrap(),
            "\"registered\""
        );
    }

    #[test]
    fn test_clear_registered_models_is_noop() {
        // clear_registered_models が正常に呼び出せることを確認（no-op）
        clear_registered_models();
        // パニックしなければ成功
    }
}
