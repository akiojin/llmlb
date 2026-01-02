# データモデル: APIキースコープシステム

## エンティティ定義

### ApiKeyScope

APIキーに付与できるスコープ。

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyScope {
    /// ノード登録・ハートビート用
    Node,
    /// OpenAI互換API利用用
    Api,
    /// 管理操作用（全権限を包含）
    Admin,
}
```

### ApiKey

APIキーエンティティ。

```rust
pub struct ApiKey {
    pub id: Uuid,
    pub name: String,
    pub key_hash: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub scopes: Vec<ApiKeyScope>,
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

## スコープとエンドポイントのマッピング

| エンドポイント | 必要なスコープ |
|---------------|---------------|
| POST /v0/nodes | `node` |
| GET /v0/health | `node` + ノードトークン |
| GET /v0/models/registry/:name/manifest.json | `node` |
| POST /v1/chat/completions | `api` |
| POST /v1/embeddings | `api` |
| GET /v1/models | `api` |
| POST /v1/models/register | `admin` |
| DELETE /v1/models/:name | `admin` |
| GET /v0/users | `admin` |
| GET /v0/api-keys | `admin` |
| GET /v0/metrics/* | `admin` |
| GET /v0/dashboard/* | `admin` または JWT |

## デバッグAPIキー

開発モード（`#[cfg(debug_assertions)]`）で有効。

| キー | スコープ | 用途 |
|------|---------|------|
| `sk_debug` | すべて | 後方互換性 |
| `sk_debug_node` | `node` | ノード登録テスト |
| `sk_debug_api` | `api` | API利用テスト |
| `sk_debug_admin` | `admin` | 管理操作テスト |

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
    scopes TEXT NOT NULL DEFAULT '["api"]'
);
```

### scopesカラム

JSON配列として格納。

```json
["node", "api"]
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
[スコープ確認]
     |
     +-- 権限なし → 403 Forbidden
     |
     v
[リクエスト処理]
```

### ノードヘルスチェック認証

```text
[リクエスト]
     |
     v
[Authorization: Bearer sk_xxxx]
[X-Node-Token: node_xxxx]
     |
     v
[APIキー検証（nodeスコープ）]
     |
     +-- 失敗 → 401/403
     |
     v
[ノードトークン検証]
     |
     +-- 失敗 → 401 Unauthorized
     |
     v
[ヘルスチェック処理]
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
    "message": "API key does not have required scope: node",
    "type": "forbidden",
    "code": "insufficient_scope"
  }
}
```

## 後方互換性

スコープが未設定のAPIキーは全スコープとして扱う。

```rust
fn get_effective_scopes(api_key: &ApiKey) -> Vec<ApiKeyScope> {
    if api_key.scopes.is_empty() {
        vec![ApiKeyScope::Node, ApiKeyScope::Api, ApiKeyScope::Admin]
    } else {
        api_key.scopes.clone()
    }
}
```
