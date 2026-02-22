//! 監査ログの型定義 (SPEC-8301d106)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// アクター種別
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    /// JWT認証ユーザー
    User,
    /// APIキー認証
    ApiKey,
    /// 未認証（認証失敗含む）
    Anonymous,
}

impl ActorType {
    /// 文字列からActorTypeに変換
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "user" => Self::User,
            "api_key" => Self::ApiKey,
            _ => Self::Anonymous,
        }
    }

    /// ActorTypeを文字列に変換
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::ApiKey => "api_key",
            Self::Anonymous => "anonymous",
        }
    }
}

impl std::fmt::Display for ActorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 監査ログエントリ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// レコードID（DB挿入後に設定）
    pub id: Option<i64>,
    /// タイムスタンプ
    pub timestamp: DateTime<Utc>,
    /// HTTPメソッド
    pub http_method: String,
    /// リクエストパス
    pub request_path: String,
    /// HTTPステータスコード
    pub status_code: u16,
    /// アクター種別
    pub actor_type: ActorType,
    /// アクターID（user_id or api_key_id）
    pub actor_id: Option<String>,
    /// ユーザー名（表示用）
    pub actor_username: Option<String>,
    /// APIキー発行者のuser_id
    pub api_key_owner_id: Option<String>,
    /// クライアントIPアドレス
    pub client_ip: Option<String>,
    /// リクエスト処理時間（ミリ秒）
    pub duration_ms: Option<i64>,
    /// 入力トークン数（推論リクエストのみ）
    pub input_tokens: Option<i64>,
    /// 出力トークン数（推論リクエストのみ）
    pub output_tokens: Option<i64>,
    /// 合計トークン数（推論リクエストのみ）
    pub total_tokens: Option<i64>,
    /// モデル名（推論リクエストのみ）
    pub model_name: Option<String>,
    /// エンドポイントID（推論リクエストのみ）
    pub endpoint_id: Option<String>,
    /// 操作の追加情報（JSON）
    pub detail: Option<String>,
    /// 所属バッチID
    pub batch_id: Option<i64>,
    /// request_historyからの移行データフラグ
    pub is_migrated: bool,
}

/// バッチハッシュ（改ざん防止チェーン）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditBatchHash {
    /// バッチID
    pub id: Option<i64>,
    /// バッチ連番
    pub sequence_number: i64,
    /// バッチ開始時刻
    pub batch_start: DateTime<Utc>,
    /// バッチ終了時刻
    pub batch_end: DateTime<Utc>,
    /// バッチ内レコード数
    pub record_count: i64,
    /// SHA-256ハッシュ値
    pub hash: String,
    /// 前バッチのハッシュ値
    pub previous_hash: String,
}

/// 監査ログフィルタ
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditLogFilter {
    /// アクター種別でフィルタ
    pub actor_type: Option<String>,
    /// アクターIDでフィルタ
    pub actor_id: Option<String>,
    /// HTTPメソッドでフィルタ
    pub http_method: Option<String>,
    /// リクエストパスでフィルタ
    pub request_path: Option<String>,
    /// ステータスコードでフィルタ
    pub status_code: Option<u16>,
    /// 開始日時
    pub time_from: Option<DateTime<Utc>>,
    /// 終了日時
    pub time_to: Option<DateTime<Utc>>,
    /// フリーテキスト検索
    pub search_text: Option<String>,
    /// ページ番号（1始まり）
    pub page: Option<i64>,
    /// ページあたり件数
    pub per_page: Option<i64>,
    /// アーカイブを含むか
    pub include_archive: Option<bool>,
}

/// 推論リクエストのトークン使用量（ハンドラーからミドルウェアへの受け渡し用）
#[derive(Debug, Clone)]
pub struct TokenUsage {
    /// 入力トークン数
    pub input_tokens: Option<i64>,
    /// 出力トークン数
    pub output_tokens: Option<i64>,
    /// 合計トークン数
    pub total_tokens: Option<i64>,
    /// モデル名
    pub model_name: Option<String>,
    /// エンドポイントID
    pub endpoint_id: Option<String>,
}

/// 認証失敗情報（認証ミドルウェアから監査ミドルウェアへの受け渡し用）
#[derive(Debug, Clone)]
pub struct AuthFailureInfo {
    /// 試行されたユーザー名
    pub attempted_username: Option<String>,
    /// 失敗理由
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_actor_type_serialization() {
        assert_eq!(serde_json::to_string(&ActorType::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&ActorType::ApiKey).unwrap(),
            "\"api_key\""
        );
        assert_eq!(
            serde_json::to_string(&ActorType::Anonymous).unwrap(),
            "\"anonymous\""
        );
    }

    #[test]
    fn test_actor_type_deserialization() {
        let user: ActorType = serde_json::from_str("\"user\"").unwrap();
        assert_eq!(user, ActorType::User);
        let api_key: ActorType = serde_json::from_str("\"api_key\"").unwrap();
        assert_eq!(api_key, ActorType::ApiKey);
        let anon: ActorType = serde_json::from_str("\"anonymous\"").unwrap();
        assert_eq!(anon, ActorType::Anonymous);
    }

    #[test]
    fn test_actor_type_from_str() {
        assert_eq!(ActorType::from_str("user"), ActorType::User);
        assert_eq!(ActorType::from_str("api_key"), ActorType::ApiKey);
        assert_eq!(ActorType::from_str("anonymous"), ActorType::Anonymous);
        assert_eq!(ActorType::from_str("unknown"), ActorType::Anonymous);
    }

    #[test]
    fn test_actor_type_as_str() {
        assert_eq!(ActorType::User.as_str(), "user");
        assert_eq!(ActorType::ApiKey.as_str(), "api_key");
        assert_eq!(ActorType::Anonymous.as_str(), "anonymous");
    }

    #[test]
    fn test_audit_log_entry_creation() {
        let entry = AuditLogEntry {
            id: None,
            timestamp: Utc::now(),
            http_method: "GET".to_string(),
            request_path: "/api/users".to_string(),
            status_code: 200,
            actor_type: ActorType::User,
            actor_id: Some("user-123".to_string()),
            actor_username: Some("admin".to_string()),
            api_key_owner_id: None,
            client_ip: Some("127.0.0.1".to_string()),
            duration_ms: Some(42),
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            model_name: None,
            endpoint_id: None,
            detail: None,
            batch_id: None,
            is_migrated: false,
        };
        assert_eq!(entry.http_method, "GET");
        assert_eq!(entry.status_code, 200);
        assert_eq!(entry.actor_type, ActorType::User);
        assert!(!entry.is_migrated);
    }

    #[test]
    fn test_audit_log_entry_serialization() {
        let entry = AuditLogEntry {
            id: Some(1),
            timestamp: Utc::now(),
            http_method: "POST".to_string(),
            request_path: "/v1/chat/completions".to_string(),
            status_code: 200,
            actor_type: ActorType::ApiKey,
            actor_id: Some("key-456".to_string()),
            actor_username: None,
            api_key_owner_id: Some("user-789".to_string()),
            client_ip: Some("10.0.0.1".to_string()),
            duration_ms: Some(1500),
            input_tokens: Some(100),
            output_tokens: Some(50),
            total_tokens: Some(150),
            model_name: Some("llama-3".to_string()),
            endpoint_id: Some("ep-1".to_string()),
            detail: None,
            batch_id: Some(1),
            is_migrated: false,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"actor_type\":\"api_key\""));
        assert!(json.contains("\"total_tokens\":150"));
    }

    #[test]
    fn test_audit_batch_hash_creation() {
        let batch = AuditBatchHash {
            id: None,
            sequence_number: 1,
            batch_start: Utc::now(),
            batch_end: Utc::now(),
            record_count: 42,
            hash: "abc123".to_string(),
            previous_hash: "0".repeat(64),
        };
        assert_eq!(batch.sequence_number, 1);
        assert_eq!(batch.record_count, 42);
        assert_eq!(batch.previous_hash.len(), 64);
    }

    #[test]
    fn test_audit_log_filter_default() {
        let filter = AuditLogFilter::default();
        assert!(filter.actor_type.is_none());
        assert!(filter.page.is_none());
        assert!(filter.per_page.is_none());
        assert!(filter.search_text.is_none());
    }
}
