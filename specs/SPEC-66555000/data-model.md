# データモデル: ロードバランサー主導エンドポイント登録システム

**機能ID**: `SPEC-66555000`
**日付**: 2026-01-14

## エンティティ一覧

### 1. Endpoint（エンドポイント）

推論サービスの接続先を表すエンティティ。

**フィールド**:

| フィールド | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| id | UUID | Yes | 一意識別子 |
| name | String | Yes | 表示名（例: "本番Ollama", "開発xLLM1"） |
| base_url | String | Yes | ベースURL（例: `http://192.168.1.100:11434`） |
| api_key | String? | No | APIキー（暗号化保存） |
| status | EndpointStatus | Yes | 現在の状態 |
| health_check_interval_secs | u32 | Yes | ヘルスチェック間隔（デフォルト: 30） |
| inference_timeout_secs | u32 | Yes | 推論タイムアウト（デフォルト: 120） |
| latency_ms | u32? | No | ヘルスチェック時のレイテンシ（ルーティング用） |
| last_seen | DateTime? | No | 最終確認時刻 |
| last_error | String? | No | 最後のエラーメッセージ |
| error_count | u32 | Yes | 連続エラー回数 |
| registered_at | DateTime | Yes | 登録日時 |
| notes | String? | No | メモ |

**検証ルール**:

- `name`: 1-100文字、空白のみ不可、UNIQUE制約
- `base_url`: 有効なURL形式、UNIQUE制約
- `health_check_interval_secs`: 10-300の範囲
- `inference_timeout_secs`: 10-600の範囲

### 2. EndpointStatus（エンドポイント状態）

エンドポイントの状態を表す列挙型。

**値**:

| 値 | 説明 | 遷移条件 |
|----|------|---------|
| `pending` | 初期状態（未確認） | 登録直後 |
| `online` | 稼働中 | ヘルスチェック成功 |
| `offline` | 停止中 | 連続2回ヘルスチェック失敗 |
| `error` | エラー状態 | 接続エラー、認証エラー等 |

**状態遷移図**:

```text
[登録] → pending
          ↓ ヘルスチェック成功
        online ←→ offline
          ↓ ↑
        error
```

### 3. EndpointModel（エンドポイントモデル）

エンドポイントで利用可能なモデル情報。

**フィールド**:

| フィールド | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| endpoint_id | UUID | Yes | エンドポイントID（FK） |
| model_id | String | Yes | モデル識別子 |
| capabilities | Vec\<String\>? | No | 能力（chat, embeddings等） |
| last_checked | DateTime? | No | 最終確認時刻 |

**複合主キー**: (endpoint_id, model_id)

### 4. EndpointHealthCheck（ヘルスチェック履歴）

ヘルスチェックの履歴を記録するエンティティ。

**フィールド**:

| フィールド | 型 | 必須 | 説明 |
|-----------|-----|------|------|
| id | i64 | Yes | 自動インクリメントID |
| endpoint_id | UUID | Yes | エンドポイントID（FK） |
| checked_at | DateTime | Yes | チェック実行時刻 |
| success | bool | Yes | 成功/失敗 |
| latency_ms | u32? | No | レイテンシ（成功時のみ） |
| error_message | String? | No | エラーメッセージ（失敗時のみ） |
| status_before | EndpointStatus | Yes | チェック前の状態 |
| status_after | EndpointStatus | Yes | チェック後の状態 |

**保持期間**: 30日間（自動削除）

## データベーススキーマ

### endpoints テーブル

```sql
CREATE TABLE endpoints (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    base_url TEXT NOT NULL UNIQUE,
    api_key_encrypted TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    health_check_interval_secs INTEGER NOT NULL DEFAULT 30,
    inference_timeout_secs INTEGER NOT NULL DEFAULT 120,
    latency_ms INTEGER,
    last_seen TEXT,
    last_error TEXT,
    error_count INTEGER NOT NULL DEFAULT 0,
    registered_at TEXT NOT NULL,
    notes TEXT
);

CREATE INDEX idx_endpoints_status ON endpoints(status);
CREATE INDEX idx_endpoints_name ON endpoints(name);
```

### endpoint_models テーブル

```sql
CREATE TABLE endpoint_models (
    endpoint_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    capabilities TEXT,  -- JSON: ["chat", "embeddings"]
    last_checked TEXT,
    PRIMARY KEY (endpoint_id, model_id),
    FOREIGN KEY (endpoint_id) REFERENCES endpoints(id) ON DELETE CASCADE
);

CREATE INDEX idx_endpoint_models_model ON endpoint_models(model_id);
```

### endpoint_health_checks テーブル

```sql
CREATE TABLE endpoint_health_checks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    endpoint_id TEXT NOT NULL,
    checked_at TEXT NOT NULL,
    success INTEGER NOT NULL,
    latency_ms INTEGER,
    error_message TEXT,
    status_before TEXT NOT NULL,
    status_after TEXT NOT NULL,
    FOREIGN KEY (endpoint_id) REFERENCES endpoints(id) ON DELETE CASCADE
);

CREATE INDEX idx_health_checks_endpoint ON endpoint_health_checks(endpoint_id);
CREATE INDEX idx_health_checks_checked_at ON endpoint_health_checks(checked_at);
```

## Rust構造体定義

```rust
// llmlb/src/types/endpoint.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EndpointStatus {
    Pending,
    Online,
    Offline,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub id: Uuid,
    pub name: String,
    pub base_url: String,
    #[serde(skip_serializing)]
    pub api_key: Option<String>,
    pub status: EndpointStatus,
    pub health_check_interval_secs: u32,
    pub inference_timeout_secs: u32,
    pub latency_ms: Option<u32>,
    pub last_seen: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub error_count: u32,
    pub registered_at: DateTime<Utc>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointModel {
    pub endpoint_id: Uuid,
    pub model_id: String,
    pub capabilities: Option<Vec<String>>,
    pub last_checked: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointHealthCheck {
    pub id: i64,
    pub endpoint_id: Uuid,
    pub checked_at: DateTime<Utc>,
    pub success: bool,
    pub latency_ms: Option<u32>,
    pub error_message: Option<String>,
    pub status_before: EndpointStatus,
    pub status_after: EndpointStatus,
}
```

## 既存エンティティとの関係

### 廃止されるエンティティ

- `Node`: `Endpoint`に置換
- `NodeStatus`: `EndpointStatus`に置換
- `RegisterRequest/RegisterResponse`: 廃止（ロードバランサー主導登録に変更）

### 維持されるエンティティ

- `Model`: モデルカタログ（変更なし）
- `User`: ユーザー管理（変更なし）
- `ApiKey`: APIキー管理（変更なし）

## 移行計画

1. **Phase 1**: `endpoints`/`endpoint_models`テーブル追加
2. **Phase 2**: `EndpointRegistry`実装、既存`NodeRegistry`と並行運用
3. **Phase 3**: ルーティングを`EndpointRegistry`に切り替え
4. **Phase 4**: `nodes`テーブル・`NodeRegistry`削除

---

*データモデル設計完了*
