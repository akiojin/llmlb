// T032-T037: 認証関連のデータモデル（最小実装、テスト用）

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// ユーザーロール
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    /// 管理者（全操作可能）
    Admin,
    /// 閲覧者（読み取りのみ）
    Viewer,
}

/// ユーザー
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// ユーザーID
    pub id: Uuid,
    /// ユーザー名
    pub username: String,
    /// パスワードハッシュ（bcrypt）
    pub password_hash: String,
    /// ユーザーロール
    pub role: UserRole,
    /// 作成日時
    pub created_at: DateTime<Utc>,
    /// 最終ログイン日時
    pub last_login: Option<DateTime<Utc>>,
}

/// APIキー
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// APIキーID
    pub id: Uuid,
    /// キーのSHA-256ハッシュ
    pub key_hash: String,
    /// キーの先頭部分（表示用）
    pub key_prefix: Option<String>,
    /// キーの名前
    pub name: String,
    /// 作成者のユーザーID
    pub created_by: Uuid,
    /// 作成日時
    pub created_at: DateTime<Utc>,
    /// 有効期限
    pub expires_at: Option<DateTime<Utc>>,
    /// スコープ（権限）
    pub scopes: Vec<ApiKeyScope>,
}

/// APIキースコープ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApiKeyScope {
    /// エンドポイント登録・同期
    #[serde(rename = "endpoint")]
    Endpoint,
    /// OpenAI互換API利用
    #[serde(rename = "api")]
    Api,
    /// 管理者（全権限）
    #[serde(rename = "admin")]
    Admin,
}

impl ApiKeyScope {
    /// すべてのスコープ
    pub fn all() -> Vec<ApiKeyScope> {
        vec![ApiKeyScope::Endpoint, ApiKeyScope::Api, ApiKeyScope::Admin]
    }
}

/// APIキー（平文付き、発行時のレスポンス用）
#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyWithPlaintext {
    /// APIキーID
    pub id: Uuid,
    /// 平文のAPIキー（発行時のみ表示）
    pub key: String,
    /// キーの先頭部分（表示用）
    pub key_prefix: String,
    /// キーの名前
    pub name: String,
    /// 作成日時
    pub created_at: DateTime<Utc>,
    /// 有効期限
    pub expires_at: Option<DateTime<Utc>>,
    /// スコープ（権限）
    pub scopes: Vec<ApiKeyScope>,
}

/// ランタイムトークン
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeToken {
    /// ランタイムID
    pub runtime_id: Uuid,
    /// トークンのSHA-256ハッシュ
    pub token_hash: String,
    /// 作成日時
    pub created_at: DateTime<Utc>,
}

/// ランタイムトークン（平文付き、発行時のレスポンス用）
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeTokenWithPlaintext {
    /// ランタイムID
    pub runtime_id: Uuid,
    /// 平文のトークン（発行時のみ表示）
    pub token: String,
    /// 作成日時
    pub created_at: DateTime<Utc>,
}

/// JWTクレーム
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Claims {
    /// ユーザーID（JWT sub claim）
    pub sub: String,
    /// ユーザーロール
    pub role: UserRole,
    /// 有効期限（Unix timestamp、JWT exp claim）
    pub exp: usize,
}
