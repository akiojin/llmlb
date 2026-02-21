//! Repository traitパターン定義
//!
//! DB操作を抽象化し、テスタビリティを向上させるためのtrait群。
//! 各traitは既存のフリー関数/構造体メソッドに対応する。

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::common::auth::{ApiKey, ApiKeyPermission, ApiKeyWithPlaintext, User, UserRole};
use crate::common::error::LbError;
use crate::common::protocol::RequestResponseRecord;
use crate::registry::models::ModelInfo;
use crate::types::endpoint::{
    DeviceInfo, Endpoint, EndpointHealthCheck, EndpointModel, EndpointStatus, EndpointType,
    ModelDownloadTask,
};

use super::endpoint_daily_stats::{DailyStatEntry, ModelStatEntry};
use super::endpoints::EndpointRequestTotals;
use super::invitations::{InvitationCode, InvitationCodeWithPlaintext};
use super::request_history::{FilteredRecords, RecordFilter, TokenStatistics};

// ---------------------------------------------------------------------------
// EndpointRepository
// ---------------------------------------------------------------------------

/// エンドポイントCRUD操作のRepository trait
#[async_trait]
pub trait EndpointRepository: Send + Sync {
    /// エンドポイントを登録
    async fn create_endpoint(&self, endpoint: &Endpoint) -> Result<(), sqlx::Error>;
    /// エンドポイント一覧を取得
    async fn list_endpoints(&self) -> Result<Vec<Endpoint>, sqlx::Error>;
    /// IDでエンドポイントを取得
    async fn get_endpoint(&self, id: Uuid) -> Result<Option<Endpoint>, sqlx::Error>;
    /// エンドポイントを更新
    async fn update_endpoint(&self, endpoint: &Endpoint) -> Result<bool, sqlx::Error>;
    /// エンドポイントを削除
    async fn delete_endpoint(&self, id: Uuid) -> Result<bool, sqlx::Error>;
    /// 名前でエンドポイントを検索
    async fn find_by_name(&self, name: &str) -> Result<Option<Endpoint>, sqlx::Error>;
    /// ステータスでフィルタしてエンドポイント一覧を取得
    async fn list_endpoints_by_status(
        &self,
        status: EndpointStatus,
    ) -> Result<Vec<Endpoint>, sqlx::Error>;
    /// タイプでフィルタしてエンドポイント一覧を取得
    async fn list_endpoints_by_type(
        &self,
        endpoint_type: EndpointType,
    ) -> Result<Vec<Endpoint>, sqlx::Error>;
    /// エンドポイントのステータスを更新
    async fn update_endpoint_status(
        &self,
        id: Uuid,
        status: EndpointStatus,
        latency_ms: Option<u32>,
        last_error: Option<&str>,
    ) -> Result<bool, sqlx::Error>;
    /// エンドポイントの推論レイテンシを更新
    async fn update_inference_latency(
        &self,
        id: Uuid,
        inference_latency_ms: Option<f64>,
    ) -> Result<bool, sqlx::Error>;
    /// エンドポイントのデバイス情報を更新
    async fn update_device_info(
        &self,
        id: Uuid,
        device_info: Option<&DeviceInfo>,
    ) -> Result<bool, sqlx::Error>;
    /// エンドポイントのリクエストカウンタをインクリメント
    async fn increment_request_counters(
        &self,
        id: Uuid,
        success: bool,
    ) -> Result<bool, sqlx::Error>;
    /// エンドポイントの累計リクエスト統計を取得
    async fn get_request_totals(&self) -> Result<EndpointRequestTotals, sqlx::Error>;
    /// エンドポイントにモデルを追加
    async fn add_endpoint_model(&self, model: &EndpointModel) -> Result<(), sqlx::Error>;
    /// エンドポイントのモデル一覧を取得
    async fn list_endpoint_models(
        &self,
        endpoint_id: Uuid,
    ) -> Result<Vec<EndpointModel>, sqlx::Error>;
    /// エンドポイントからモデルを削除
    async fn delete_endpoint_model(
        &self,
        endpoint_id: Uuid,
        model_id: &str,
    ) -> Result<bool, sqlx::Error>;
    /// エンドポイントの全モデルを削除
    async fn delete_all_endpoint_models(&self, endpoint_id: Uuid) -> Result<u64, sqlx::Error>;
    /// ヘルスチェック結果を記録
    async fn record_health_check(&self, check: &EndpointHealthCheck) -> Result<i64, sqlx::Error>;
    /// エンドポイントのヘルスチェック履歴を取得
    async fn list_health_checks(
        &self,
        endpoint_id: Uuid,
        limit: i32,
    ) -> Result<Vec<EndpointHealthCheck>, sqlx::Error>;
}

// ---------------------------------------------------------------------------
// UserRepository
// ---------------------------------------------------------------------------

/// ユーザーCRUD操作のRepository trait
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// ユーザーを作成
    async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        role: UserRole,
    ) -> Result<User, LbError>;
    /// ユーザー名でユーザーを検索
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, LbError>;
    /// IDでユーザーを検索
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, LbError>;
    /// すべてのユーザーを取得
    async fn list_users(&self) -> Result<Vec<User>, LbError>;
    /// ユーザーを更新
    async fn update_user(
        &self,
        id: Uuid,
        username: Option<&str>,
        password_hash: Option<&str>,
        role: Option<UserRole>,
    ) -> Result<User, LbError>;
    /// 最終ログイン日時を更新
    async fn update_last_login(&self, id: Uuid) -> Result<(), LbError>;
    /// ユーザーを削除
    async fn delete_user(&self, id: Uuid) -> Result<(), LbError>;
    /// 初回起動チェック
    async fn is_first_boot(&self) -> Result<bool, LbError>;
    /// 最後の管理者チェック
    async fn is_last_admin(&self, user_id: Uuid) -> Result<bool, LbError>;
}

// ---------------------------------------------------------------------------
// ApiKeyRepository
// ---------------------------------------------------------------------------

/// APIキーCRUD操作のRepository trait
#[async_trait]
pub trait ApiKeyRepository: Send + Sync {
    /// APIキーを生成
    async fn create_api_key(
        &self,
        name: &str,
        created_by: Uuid,
        expires_at: Option<DateTime<Utc>>,
        permissions: Vec<ApiKeyPermission>,
    ) -> Result<ApiKeyWithPlaintext, LbError>;
    /// ハッシュ値でAPIキーを検索
    async fn find_by_hash(&self, key_hash: &str) -> Result<Option<ApiKey>, LbError>;
    /// すべてのAPIキーを取得
    async fn list_api_keys(&self) -> Result<Vec<ApiKey>, LbError>;
    /// APIキーを削除
    async fn delete_api_key(&self, id: Uuid) -> Result<(), LbError>;
}

// ---------------------------------------------------------------------------
// InvitationRepository
// ---------------------------------------------------------------------------

/// 招待コードCRUD操作のRepository trait
#[async_trait]
pub trait InvitationRepository: Send + Sync {
    /// 招待コードを生成
    async fn create_invitation(
        &self,
        created_by: Uuid,
        expires_in_hours: Option<i64>,
    ) -> Result<InvitationCodeWithPlaintext, LbError>;
    /// 平文コードから招待コードを検索
    async fn find_by_code(&self, plaintext_code: &str) -> Result<Option<InvitationCode>, LbError>;
    /// すべての招待コードを取得
    async fn list_invitations(&self) -> Result<Vec<InvitationCode>, LbError>;
    /// 招待コードを使用済みにする
    async fn mark_as_used(&self, id: Uuid, used_by: Uuid) -> Result<(), LbError>;
    /// 招待コードを無効化
    async fn revoke(&self, id: Uuid) -> Result<bool, LbError>;
    /// 招待コードを削除
    async fn delete_invitation(&self, id: Uuid) -> Result<(), LbError>;
}

// ---------------------------------------------------------------------------
// RequestHistoryRepository
// ---------------------------------------------------------------------------

/// リクエスト履歴操作のRepository trait
#[async_trait]
pub trait RequestHistoryRepository: Send + Sync {
    /// レコードを保存
    async fn save_record(
        &self,
        record: &RequestResponseRecord,
    ) -> crate::common::error::RouterResult<()>;
    /// すべてのレコードを読み込み
    async fn load_records(&self) -> crate::common::error::RouterResult<Vec<RequestResponseRecord>>;
    /// 指定期間より古いレコードを削除
    async fn cleanup_old_records(
        &self,
        max_age: Duration,
    ) -> crate::common::error::RouterResult<()>;
    /// レコードをフィルタリング＆ページネーション
    async fn filter_and_paginate(
        &self,
        filter: &RecordFilter,
        page: usize,
        per_page: usize,
    ) -> crate::common::error::RouterResult<FilteredRecords>;
    /// トークン統計を取得
    async fn get_token_statistics(&self) -> crate::common::error::RouterResult<TokenStatistics>;
}

// ---------------------------------------------------------------------------
// ModelRepository
// ---------------------------------------------------------------------------

/// モデル情報操作のRepository trait
#[async_trait]
pub trait ModelRepository: Send + Sync {
    /// モデルを保存
    async fn save_model(&self, model: &ModelInfo) -> crate::common::error::RouterResult<()>;
    /// 全モデルを読み込み
    async fn load_models(&self) -> crate::common::error::RouterResult<Vec<ModelInfo>>;
    /// 特定のモデルを読み込み
    async fn load_model(&self, name: &str)
        -> crate::common::error::RouterResult<Option<ModelInfo>>;
    /// モデルを削除
    async fn delete_model(&self, name: &str) -> crate::common::error::RouterResult<()>;
}

// ---------------------------------------------------------------------------
// EndpointDailyStatsRepository
// ---------------------------------------------------------------------------

/// エンドポイント日次統計操作のRepository trait
#[async_trait]
pub trait EndpointDailyStatsRepository: Send + Sync {
    /// 日次統計をUPSERT
    async fn upsert_daily_stats(
        &self,
        endpoint_id: Uuid,
        model_id: &str,
        date: &str,
        success: bool,
        output_tokens: u64,
        duration_ms: u64,
    ) -> Result<(), sqlx::Error>;
    /// 日次集計データを取得
    async fn get_daily_stats(
        &self,
        endpoint_id: Uuid,
        days: u32,
    ) -> Result<Vec<DailyStatEntry>, sqlx::Error>;
    /// モデル別集計データを取得
    async fn get_model_stats(&self, endpoint_id: Uuid) -> Result<Vec<ModelStatEntry>, sqlx::Error>;
    /// 当日の集計データを取得
    async fn get_today_stats(
        &self,
        endpoint_id: Uuid,
        today: &str,
    ) -> Result<DailyStatEntry, sqlx::Error>;
}

// ---------------------------------------------------------------------------
// DownloadTaskRepository
// ---------------------------------------------------------------------------

/// ダウンロードタスク操作のRepository trait
#[async_trait]
pub trait DownloadTaskRepository: Send + Sync {
    /// ダウンロードタスクを作成
    async fn create_download_task(&self, task: &ModelDownloadTask) -> Result<(), sqlx::Error>;
    /// ダウンロードタスクをIDで取得
    async fn get_download_task(
        &self,
        task_id: &str,
    ) -> Result<Option<ModelDownloadTask>, sqlx::Error>;
    /// エンドポイントのダウンロードタスク一覧を取得
    async fn list_download_tasks(
        &self,
        endpoint_id: Uuid,
    ) -> Result<Vec<ModelDownloadTask>, sqlx::Error>;
    /// アクティブなダウンロードタスク一覧を取得
    async fn list_active_download_tasks(
        &self,
        endpoint_id: Uuid,
    ) -> Result<Vec<ModelDownloadTask>, sqlx::Error>;
    /// ダウンロード進捗を更新
    async fn update_download_progress(
        &self,
        task_id: &str,
        progress: f64,
        speed_mbps: Option<f64>,
        eta_seconds: Option<u32>,
    ) -> Result<bool, sqlx::Error>;
    /// ダウンロードタスクを完了にする
    async fn complete_download_task(
        &self,
        task_id: &str,
        filename: Option<&str>,
    ) -> Result<bool, sqlx::Error>;
    /// ダウンロードタスクを失敗にする
    async fn fail_download_task(
        &self,
        task_id: &str,
        error_message: &str,
    ) -> Result<bool, sqlx::Error>;
    /// ダウンロードタスクをキャンセルする
    async fn cancel_download_task(&self, task_id: &str) -> Result<bool, sqlx::Error>;
    /// ダウンロードタスクを削除する
    async fn delete_download_task(&self, task_id: &str) -> Result<bool, sqlx::Error>;
}

// ---------------------------------------------------------------------------
// SettingsRepository
// ---------------------------------------------------------------------------

/// 設定管理のRepository trait
#[async_trait]
pub trait SettingsRepository: Send + Sync {
    /// 設定値を取得
    async fn get_setting(&self, key: &str) -> crate::common::error::RouterResult<Option<String>>;
    /// 設定値を保存
    async fn set_setting(&self, key: &str, value: &str) -> crate::common::error::RouterResult<()>;
}

// ===========================================================================
// SqlitePool implementations
// ===========================================================================

use sqlx::SqlitePool;

#[async_trait]
impl EndpointRepository for SqlitePool {
    async fn create_endpoint(&self, endpoint: &Endpoint) -> Result<(), sqlx::Error> {
        super::endpoints::create_endpoint(self, endpoint).await
    }

    async fn list_endpoints(&self) -> Result<Vec<Endpoint>, sqlx::Error> {
        super::endpoints::list_endpoints(self).await
    }

    async fn get_endpoint(&self, id: Uuid) -> Result<Option<Endpoint>, sqlx::Error> {
        super::endpoints::get_endpoint(self, id).await
    }

    async fn update_endpoint(&self, endpoint: &Endpoint) -> Result<bool, sqlx::Error> {
        super::endpoints::update_endpoint(self, endpoint).await
    }

    async fn delete_endpoint(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        super::endpoints::delete_endpoint(self, id).await
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Endpoint>, sqlx::Error> {
        super::endpoints::find_by_name(self, name).await
    }

    async fn list_endpoints_by_status(
        &self,
        status: EndpointStatus,
    ) -> Result<Vec<Endpoint>, sqlx::Error> {
        super::endpoints::list_endpoints_by_status(self, status).await
    }

    async fn list_endpoints_by_type(
        &self,
        endpoint_type: EndpointType,
    ) -> Result<Vec<Endpoint>, sqlx::Error> {
        super::endpoints::list_endpoints_by_type(self, endpoint_type).await
    }

    async fn update_endpoint_status(
        &self,
        id: Uuid,
        status: EndpointStatus,
        latency_ms: Option<u32>,
        last_error: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        super::endpoints::update_endpoint_status(self, id, status, latency_ms, last_error).await
    }

    async fn update_inference_latency(
        &self,
        id: Uuid,
        inference_latency_ms: Option<f64>,
    ) -> Result<bool, sqlx::Error> {
        super::endpoints::update_inference_latency(self, id, inference_latency_ms).await
    }

    async fn update_device_info(
        &self,
        id: Uuid,
        device_info: Option<&DeviceInfo>,
    ) -> Result<bool, sqlx::Error> {
        super::endpoints::update_device_info(self, id, device_info).await
    }

    async fn increment_request_counters(
        &self,
        id: Uuid,
        success: bool,
    ) -> Result<bool, sqlx::Error> {
        super::endpoints::increment_request_counters(self, id, success).await
    }

    async fn get_request_totals(&self) -> Result<EndpointRequestTotals, sqlx::Error> {
        super::endpoints::get_request_totals(self).await
    }

    async fn add_endpoint_model(&self, model: &EndpointModel) -> Result<(), sqlx::Error> {
        super::endpoints::add_endpoint_model(self, model).await
    }

    async fn list_endpoint_models(
        &self,
        endpoint_id: Uuid,
    ) -> Result<Vec<EndpointModel>, sqlx::Error> {
        super::endpoints::list_endpoint_models(self, endpoint_id).await
    }

    async fn delete_endpoint_model(
        &self,
        endpoint_id: Uuid,
        model_id: &str,
    ) -> Result<bool, sqlx::Error> {
        super::endpoints::delete_endpoint_model(self, endpoint_id, model_id).await
    }

    async fn delete_all_endpoint_models(&self, endpoint_id: Uuid) -> Result<u64, sqlx::Error> {
        super::endpoints::delete_all_endpoint_models(self, endpoint_id).await
    }

    async fn record_health_check(&self, check: &EndpointHealthCheck) -> Result<i64, sqlx::Error> {
        super::endpoints::record_health_check(self, check).await
    }

    async fn list_health_checks(
        &self,
        endpoint_id: Uuid,
        limit: i32,
    ) -> Result<Vec<EndpointHealthCheck>, sqlx::Error> {
        super::endpoints::list_health_checks(self, endpoint_id, limit).await
    }
}

#[async_trait]
impl UserRepository for SqlitePool {
    async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        role: UserRole,
    ) -> Result<User, LbError> {
        super::users::create(self, username, password_hash, role).await
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>, LbError> {
        super::users::find_by_username(self, username).await
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, LbError> {
        super::users::find_by_id(self, id).await
    }

    async fn list_users(&self) -> Result<Vec<User>, LbError> {
        super::users::list(self).await
    }

    async fn update_user(
        &self,
        id: Uuid,
        username: Option<&str>,
        password_hash: Option<&str>,
        role: Option<UserRole>,
    ) -> Result<User, LbError> {
        super::users::update(self, id, username, password_hash, role).await
    }

    async fn update_last_login(&self, id: Uuid) -> Result<(), LbError> {
        super::users::update_last_login(self, id).await
    }

    async fn delete_user(&self, id: Uuid) -> Result<(), LbError> {
        super::users::delete(self, id).await
    }

    async fn is_first_boot(&self) -> Result<bool, LbError> {
        super::users::is_first_boot(self).await
    }

    async fn is_last_admin(&self, user_id: Uuid) -> Result<bool, LbError> {
        super::users::is_last_admin(self, user_id).await
    }
}

#[async_trait]
impl ApiKeyRepository for SqlitePool {
    async fn create_api_key(
        &self,
        name: &str,
        created_by: Uuid,
        expires_at: Option<DateTime<Utc>>,
        permissions: Vec<ApiKeyPermission>,
    ) -> Result<ApiKeyWithPlaintext, LbError> {
        super::api_keys::create(self, name, created_by, expires_at, permissions).await
    }

    async fn find_by_hash(&self, key_hash: &str) -> Result<Option<ApiKey>, LbError> {
        super::api_keys::find_by_hash(self, key_hash).await
    }

    async fn list_api_keys(&self) -> Result<Vec<ApiKey>, LbError> {
        super::api_keys::list(self).await
    }

    async fn delete_api_key(&self, id: Uuid) -> Result<(), LbError> {
        super::api_keys::delete(self, id).await
    }
}

#[async_trait]
impl InvitationRepository for SqlitePool {
    async fn create_invitation(
        &self,
        created_by: Uuid,
        expires_in_hours: Option<i64>,
    ) -> Result<InvitationCodeWithPlaintext, LbError> {
        super::invitations::create(self, created_by, expires_in_hours).await
    }

    async fn find_by_code(&self, plaintext_code: &str) -> Result<Option<InvitationCode>, LbError> {
        super::invitations::find_by_code(self, plaintext_code).await
    }

    async fn list_invitations(&self) -> Result<Vec<InvitationCode>, LbError> {
        super::invitations::list(self).await
    }

    async fn mark_as_used(&self, id: Uuid, used_by: Uuid) -> Result<(), LbError> {
        super::invitations::mark_as_used(self, id, used_by).await
    }

    async fn revoke(&self, id: Uuid) -> Result<bool, LbError> {
        super::invitations::revoke(self, id).await
    }

    async fn delete_invitation(&self, id: Uuid) -> Result<(), LbError> {
        super::invitations::delete(self, id).await
    }
}

#[async_trait]
impl RequestHistoryRepository for super::request_history::RequestHistoryStorage {
    async fn save_record(
        &self,
        record: &RequestResponseRecord,
    ) -> crate::common::error::RouterResult<()> {
        self.save_record(record).await
    }

    async fn load_records(&self) -> crate::common::error::RouterResult<Vec<RequestResponseRecord>> {
        self.load_records().await
    }

    async fn cleanup_old_records(
        &self,
        max_age: Duration,
    ) -> crate::common::error::RouterResult<()> {
        self.cleanup_old_records(max_age).await
    }

    async fn filter_and_paginate(
        &self,
        filter: &RecordFilter,
        page: usize,
        per_page: usize,
    ) -> crate::common::error::RouterResult<FilteredRecords> {
        self.filter_and_paginate(filter, page, per_page).await
    }

    async fn get_token_statistics(&self) -> crate::common::error::RouterResult<TokenStatistics> {
        self.get_token_statistics().await
    }
}

#[async_trait]
impl ModelRepository for super::models::ModelStorage {
    async fn save_model(&self, model: &ModelInfo) -> crate::common::error::RouterResult<()> {
        self.save_model(model).await
    }

    async fn load_models(&self) -> crate::common::error::RouterResult<Vec<ModelInfo>> {
        self.load_models().await
    }

    async fn load_model(
        &self,
        name: &str,
    ) -> crate::common::error::RouterResult<Option<ModelInfo>> {
        self.load_model(name).await
    }

    async fn delete_model(&self, name: &str) -> crate::common::error::RouterResult<()> {
        self.delete_model(name).await
    }
}

#[async_trait]
impl EndpointDailyStatsRepository for SqlitePool {
    async fn upsert_daily_stats(
        &self,
        endpoint_id: Uuid,
        model_id: &str,
        date: &str,
        success: bool,
        output_tokens: u64,
        duration_ms: u64,
    ) -> Result<(), sqlx::Error> {
        super::endpoint_daily_stats::upsert_daily_stats(
            self,
            endpoint_id,
            model_id,
            date,
            success,
            output_tokens,
            duration_ms,
        )
        .await
    }

    async fn get_daily_stats(
        &self,
        endpoint_id: Uuid,
        days: u32,
    ) -> Result<Vec<DailyStatEntry>, sqlx::Error> {
        super::endpoint_daily_stats::get_daily_stats(self, endpoint_id, days).await
    }

    async fn get_model_stats(&self, endpoint_id: Uuid) -> Result<Vec<ModelStatEntry>, sqlx::Error> {
        super::endpoint_daily_stats::get_model_stats(self, endpoint_id).await
    }

    async fn get_today_stats(
        &self,
        endpoint_id: Uuid,
        today: &str,
    ) -> Result<DailyStatEntry, sqlx::Error> {
        super::endpoint_daily_stats::get_today_stats(self, endpoint_id, today).await
    }
}

#[async_trait]
impl DownloadTaskRepository for SqlitePool {
    async fn create_download_task(&self, task: &ModelDownloadTask) -> Result<(), sqlx::Error> {
        super::download_tasks::create_download_task(self, task).await
    }

    async fn get_download_task(
        &self,
        task_id: &str,
    ) -> Result<Option<ModelDownloadTask>, sqlx::Error> {
        super::download_tasks::get_download_task(self, task_id).await
    }

    async fn list_download_tasks(
        &self,
        endpoint_id: Uuid,
    ) -> Result<Vec<ModelDownloadTask>, sqlx::Error> {
        super::download_tasks::list_download_tasks(self, endpoint_id).await
    }

    async fn list_active_download_tasks(
        &self,
        endpoint_id: Uuid,
    ) -> Result<Vec<ModelDownloadTask>, sqlx::Error> {
        super::download_tasks::list_active_download_tasks(self, endpoint_id).await
    }

    async fn update_download_progress(
        &self,
        task_id: &str,
        progress: f64,
        speed_mbps: Option<f64>,
        eta_seconds: Option<u32>,
    ) -> Result<bool, sqlx::Error> {
        super::download_tasks::update_download_progress(
            self,
            task_id,
            progress,
            speed_mbps,
            eta_seconds,
        )
        .await
    }

    async fn complete_download_task(
        &self,
        task_id: &str,
        filename: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        super::download_tasks::complete_download_task(self, task_id, filename).await
    }

    async fn fail_download_task(
        &self,
        task_id: &str,
        error_message: &str,
    ) -> Result<bool, sqlx::Error> {
        super::download_tasks::fail_download_task(self, task_id, error_message).await
    }

    async fn cancel_download_task(&self, task_id: &str) -> Result<bool, sqlx::Error> {
        super::download_tasks::cancel_download_task(self, task_id).await
    }

    async fn delete_download_task(&self, task_id: &str) -> Result<bool, sqlx::Error> {
        super::download_tasks::delete_download_task(self, task_id).await
    }
}

#[async_trait]
impl SettingsRepository for super::settings::SettingsStorage {
    async fn get_setting(&self, key: &str) -> crate::common::error::RouterResult<Option<String>> {
        self.get_setting(key).await
    }

    async fn set_setting(&self, key: &str, value: &str) -> crate::common::error::RouterResult<()> {
        self.set_setting(key, value).await
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // -----------------------------------------------------------------------
    // Mock UserRepository
    // -----------------------------------------------------------------------

    struct MockUserRepository {
        users: Mutex<HashMap<Uuid, User>>,
    }

    impl MockUserRepository {
        fn new() -> Self {
            Self {
                users: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl UserRepository for MockUserRepository {
        async fn create_user(
            &self,
            username: &str,
            password_hash: &str,
            role: UserRole,
        ) -> Result<User, LbError> {
            let user = User {
                id: Uuid::new_v4(),
                username: username.to_string(),
                password_hash: password_hash.to_string(),
                role,
                created_at: Utc::now(),
                last_login: None,
            };
            self.users.lock().unwrap().insert(user.id, user.clone());
            Ok(user)
        }

        async fn find_by_username(&self, username: &str) -> Result<Option<User>, LbError> {
            Ok(self
                .users
                .lock()
                .unwrap()
                .values()
                .find(|u| u.username == username)
                .cloned())
        }

        async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, LbError> {
            Ok(self.users.lock().unwrap().get(&id).cloned())
        }

        async fn list_users(&self) -> Result<Vec<User>, LbError> {
            Ok(self.users.lock().unwrap().values().cloned().collect())
        }

        async fn update_user(
            &self,
            id: Uuid,
            username: Option<&str>,
            _password_hash: Option<&str>,
            role: Option<UserRole>,
        ) -> Result<User, LbError> {
            let mut users = self.users.lock().unwrap();
            let user = users
                .get_mut(&id)
                .ok_or_else(|| LbError::Database("User not found".to_string()))?;
            if let Some(name) = username {
                user.username = name.to_string();
            }
            if let Some(r) = role {
                user.role = r;
            }
            Ok(user.clone())
        }

        async fn update_last_login(&self, _id: Uuid) -> Result<(), LbError> {
            Ok(())
        }

        async fn delete_user(&self, id: Uuid) -> Result<(), LbError> {
            self.users.lock().unwrap().remove(&id);
            Ok(())
        }

        async fn is_first_boot(&self) -> Result<bool, LbError> {
            Ok(self.users.lock().unwrap().is_empty())
        }

        async fn is_last_admin(&self, user_id: Uuid) -> Result<bool, LbError> {
            let users = self.users.lock().unwrap();
            let user = users
                .get(&user_id)
                .ok_or_else(|| LbError::Database("User not found".to_string()))?;
            if user.role != UserRole::Admin {
                return Ok(false);
            }
            let admin_count = users.values().filter(|u| u.role == UserRole::Admin).count();
            Ok(admin_count == 1)
        }
    }

    // -----------------------------------------------------------------------
    // Mock EndpointRepository
    // -----------------------------------------------------------------------

    struct MockEndpointRepository {
        endpoints: Mutex<HashMap<Uuid, Endpoint>>,
    }

    impl MockEndpointRepository {
        fn new() -> Self {
            Self {
                endpoints: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl EndpointRepository for MockEndpointRepository {
        async fn create_endpoint(&self, endpoint: &Endpoint) -> Result<(), sqlx::Error> {
            self.endpoints
                .lock()
                .unwrap()
                .insert(endpoint.id, endpoint.clone());
            Ok(())
        }

        async fn list_endpoints(&self) -> Result<Vec<Endpoint>, sqlx::Error> {
            Ok(self.endpoints.lock().unwrap().values().cloned().collect())
        }

        async fn get_endpoint(&self, id: Uuid) -> Result<Option<Endpoint>, sqlx::Error> {
            Ok(self.endpoints.lock().unwrap().get(&id).cloned())
        }

        async fn update_endpoint(&self, endpoint: &Endpoint) -> Result<bool, sqlx::Error> {
            let mut eps = self.endpoints.lock().unwrap();
            if eps.contains_key(&endpoint.id) {
                eps.insert(endpoint.id, endpoint.clone());
                Ok(true)
            } else {
                Ok(false)
            }
        }

        async fn delete_endpoint(&self, id: Uuid) -> Result<bool, sqlx::Error> {
            Ok(self.endpoints.lock().unwrap().remove(&id).is_some())
        }

        async fn find_by_name(&self, name: &str) -> Result<Option<Endpoint>, sqlx::Error> {
            Ok(self
                .endpoints
                .lock()
                .unwrap()
                .values()
                .find(|e| e.name == name)
                .cloned())
        }

        async fn list_endpoints_by_status(
            &self,
            status: EndpointStatus,
        ) -> Result<Vec<Endpoint>, sqlx::Error> {
            Ok(self
                .endpoints
                .lock()
                .unwrap()
                .values()
                .filter(|e| e.status == status)
                .cloned()
                .collect())
        }

        async fn list_endpoints_by_type(
            &self,
            endpoint_type: EndpointType,
        ) -> Result<Vec<Endpoint>, sqlx::Error> {
            Ok(self
                .endpoints
                .lock()
                .unwrap()
                .values()
                .filter(|e| e.endpoint_type == endpoint_type)
                .cloned()
                .collect())
        }

        async fn update_endpoint_status(
            &self,
            id: Uuid,
            status: EndpointStatus,
            _latency_ms: Option<u32>,
            _last_error: Option<&str>,
        ) -> Result<bool, sqlx::Error> {
            let mut eps = self.endpoints.lock().unwrap();
            if let Some(ep) = eps.get_mut(&id) {
                ep.status = status;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        async fn update_inference_latency(
            &self,
            _id: Uuid,
            _inference_latency_ms: Option<f64>,
        ) -> Result<bool, sqlx::Error> {
            Ok(true)
        }

        async fn update_device_info(
            &self,
            _id: Uuid,
            _device_info: Option<&DeviceInfo>,
        ) -> Result<bool, sqlx::Error> {
            Ok(true)
        }

        async fn increment_request_counters(
            &self,
            _id: Uuid,
            _success: bool,
        ) -> Result<bool, sqlx::Error> {
            Ok(true)
        }

        async fn get_request_totals(&self) -> Result<EndpointRequestTotals, sqlx::Error> {
            Ok(EndpointRequestTotals {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
            })
        }

        async fn add_endpoint_model(&self, _model: &EndpointModel) -> Result<(), sqlx::Error> {
            Ok(())
        }

        async fn list_endpoint_models(
            &self,
            _endpoint_id: Uuid,
        ) -> Result<Vec<EndpointModel>, sqlx::Error> {
            Ok(vec![])
        }

        async fn delete_endpoint_model(
            &self,
            _endpoint_id: Uuid,
            _model_id: &str,
        ) -> Result<bool, sqlx::Error> {
            Ok(true)
        }

        async fn delete_all_endpoint_models(&self, _endpoint_id: Uuid) -> Result<u64, sqlx::Error> {
            Ok(0)
        }

        async fn record_health_check(
            &self,
            _check: &EndpointHealthCheck,
        ) -> Result<i64, sqlx::Error> {
            Ok(1)
        }

        async fn list_health_checks(
            &self,
            _endpoint_id: Uuid,
            _limit: i32,
        ) -> Result<Vec<EndpointHealthCheck>, sqlx::Error> {
            Ok(vec![])
        }
    }

    // -----------------------------------------------------------------------
    // Test: trait as generic parameter
    // -----------------------------------------------------------------------

    async fn create_and_list_users(repo: &dyn UserRepository) -> Vec<User> {
        repo.create_user("alice", "hash1", UserRole::Admin)
            .await
            .unwrap();
        repo.create_user("bob", "hash2", UserRole::Viewer)
            .await
            .unwrap();
        repo.list_users().await.unwrap()
    }

    async fn create_and_find_endpoint(repo: &dyn EndpointRepository) -> Option<Endpoint> {
        let ep = Endpoint::new(
            "test-ep".to_string(),
            "http://localhost:8080".to_string(),
            EndpointType::Xllm,
        );
        repo.create_endpoint(&ep).await.unwrap();
        repo.get_endpoint(ep.id).await.unwrap()
    }

    #[tokio::test]
    async fn test_mock_user_repository_crud() {
        let repo = MockUserRepository::new();

        // First boot check
        assert!(repo.is_first_boot().await.unwrap());

        // Create
        let users = create_and_list_users(&repo).await;
        assert_eq!(users.len(), 2);

        // Not first boot anymore
        assert!(!repo.is_first_boot().await.unwrap());

        // Find by username
        let alice = repo.find_by_username("alice").await.unwrap();
        assert!(alice.is_some());
        assert_eq!(alice.as_ref().unwrap().role, UserRole::Admin);

        // Last admin check
        let alice_id = alice.unwrap().id;
        assert!(repo.is_last_admin(alice_id).await.unwrap());

        // Delete
        repo.delete_user(alice_id).await.unwrap();
        assert!(repo.find_by_id(alice_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_mock_endpoint_repository_crud() {
        let repo = MockEndpointRepository::new();

        // Create and find
        let ep = create_and_find_endpoint(&repo).await;
        assert!(ep.is_some());
        let ep = ep.unwrap();
        assert_eq!(ep.name, "test-ep");
        assert_eq!(ep.status, EndpointStatus::Pending);

        // List
        let all = repo.list_endpoints().await.unwrap();
        assert_eq!(all.len(), 1);

        // Update status
        repo.update_endpoint_status(ep.id, EndpointStatus::Online, Some(50), None)
            .await
            .unwrap();
        let updated = repo.get_endpoint(ep.id).await.unwrap().unwrap();
        assert_eq!(updated.status, EndpointStatus::Online);

        // Find by name
        let found = repo.find_by_name("test-ep").await.unwrap();
        assert!(found.is_some());

        // Delete
        let deleted = repo.delete_endpoint(ep.id).await.unwrap();
        assert!(deleted);
        assert!(repo.get_endpoint(ep.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_trait_object_dynamic_dispatch() {
        let repo: Box<dyn UserRepository> = Box::new(MockUserRepository::new());
        let user = repo
            .create_user("test", "hash", UserRole::Viewer)
            .await
            .unwrap();
        assert_eq!(user.username, "test");

        let found = repo.find_by_id(user.id).await.unwrap();
        assert!(found.is_some());
    }
}
