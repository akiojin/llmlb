# データモデル: APIキー権限（Permissions）システム

## エンティティ定義

### ApiKeyPermission

APIキーに付与できる権限（permissions）。

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyPermission {
    /// OpenAI互換APIの推論（/v1/chat/completions 等）
    #[serde(rename = "openai.inference")]
    OpenaiInference,
    /// OpenAI互換APIのモデル一覧取得（/v1/models*）
    #[serde(rename = "openai.models.read")]
    OpenaiModelsRead,
    /// /api/endpoints の読み取り
    #[serde(rename = "endpoints.read")]
    EndpointsRead,
    /// /api/endpoints の作成・更新・削除
    #[serde(rename = "endpoints.manage")]
    EndpointsManage,
    /// /api/api-keys の管理
    #[serde(rename = "api_keys.manage")]
    ApiKeysManage,
    /// /api/users の管理
    #[serde(rename = "users.manage")]
    UsersManage,
    /// /api/invitations の管理
    #[serde(rename = "invitations.manage")]
    InvitationsManage,
    /// /api/models/register, DELETE /api/models/* の管理
    #[serde(rename = "models.manage")]
    ModelsManage,
    /// /api/models/registry/* の読み取り
    #[serde(rename = "registry.read")]
    RegistryRead,
    /// /api/nodes/:node_id/logs の読み取り
    #[serde(rename = "logs.read")]
    LogsRead,
    /// /api/metrics/cloud の読み取り
    #[serde(rename = "metrics.read")]
    MetricsRead,
}
```

### ApiKey

APIキーエンティティ。

```rust
pub struct ApiKey {
    pub id: Uuid,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub permissions: Vec<ApiKeyPermission>,
}
```

### UserRole

ユーザーロール。

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Admin,
    Viewer,
}
```

## 権限とエンドポイントのマッピング

| エンドポイント | 必要な認証/権限 |
|---|---|
| `POST /v1/chat/completions` など（`POST /v1/*`） | APIキー + `openai.inference` |
| `GET /v1/models*` | APIキー + `openai.models.read` |
| `GET /api/endpoints*` | JWT（admin/viewer）または APIキー + `endpoints.read` |
| `POST/PUT/DELETE /api/endpoints*` | JWT（admin）または APIキー + `endpoints.manage` |
| `GET /api/users*` | JWT（admin）または APIキー + `users.manage` |
| `GET /api/api-keys*` | JWT（admin）または APIキー + `api_keys.manage` |
| `GET /api/invitations*` | JWT（admin）または APIキー + `invitations.manage` |
| `POST /api/models/register` / `DELETE /api/models/*` | JWT（admin）または APIキー + `models.manage` |
| `GET /api/models/registry/:name/manifest.json` | APIキー + `registry.read`（JWT不可） |
| `GET /api/models` / `GET /api/models/hub` | JWT（admin）または APIキー + `registry.read` |
| `GET /api/nodes/:node_id/logs` | JWT（admin）または APIキー + `logs.read` |
| `GET /api/metrics/cloud` | JWT（admin）または APIキー + `metrics.read` |
| `GET /api/dashboard/*` | JWTのみ（APIキー不可） |

## デバッグAPIキー（開発用）

開発モード（`#[cfg(debug_assertions)]`）で有効。

| キー | 権限 | 用途 |
|---|---|---|
| `sk_debug` | 全権限 | 後方互換 |
| `sk_debug_admin` | 全権限 | 管理操作テスト |
| `sk_debug_api` | `openai.inference`, `openai.models.read` | OpenAI互換APIテスト |
| `sk_debug_runtime` | `registry.read` | レジストリアクセス（旧runtime用途の置き換え） |

## データベーススキーマ

### api_keysテーブル

```sql
CREATE TABLE api_keys (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT,
    scopes TEXT, -- legacy
    permissions TEXT -- JSON array (current)
);
```

### permissionsカラム

JSON配列として格納。

```json
["openai.inference", "openai.models.read"]
```

## 認証フロー

### APIキー認証

```text
[リクエスト]
     |
     v
[Authorization: Bearer sk_xxxx]
     |
     v
[APIキー検索・検証]
     |
     +-- 見つからない → 401 Unauthorized
     +-- 期限切れ → 401 Unauthorized
     |
     v
[権限確認]
     |
     +-- 権限なし → 403 Forbidden
     |
     v
[リクエスト処理]
```

## エラーレスポンス

### 401 Unauthorized

```json
{
  "error": {
    "message": "Invalid or missing API key",
    "type": "unauthorized",
    "code": "invalid_api_key"
  }
}
```

### 403 Forbidden

```json
{
  "error": {
    "message": "Missing required permission: openai.inference",
    "type": "forbidden",
    "code": "insufficient_permission"
  }
}
```

## 後方互換性

旧`scopes`を持つ既存APIキーは、DBマイグレーションで`permissions`へ移行する。
（例: `admin` → 全権限、`api` → `openai.*`、`endpoint` → `registry.read` など）
