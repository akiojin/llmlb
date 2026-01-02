# データモデル: モデルメタデータSQLite統合

## エンティティ定義

### Model（モデル）

```rust
/// モデルメタデータ
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Model {
    /// 一意識別子（UUID）
    pub id: String,
    /// モデル名（ユニーク）
    pub name: String,
    /// ファイルサイズ（バイト）
    pub size: Option<i64>,
    /// 説明文
    pub description: Option<String>,
    /// 必要メモリ（バイト）
    pub required_memory: Option<i64>,
    /// モデルソース（hf_safetensors, hf_gguf, hf_onnx, predefined）
    pub source: Option<String>,
    /// ローカルファイルパス
    pub path: Option<String>,
    /// アーティファクト情報（JSON）
    pub artifacts_json: Option<String>,
    /// HuggingFace リポジトリID
    pub repo_id: Option<String>,
    /// 作成日時
    pub created_at: String,
    /// 更新日時
    pub updated_at: String,
}

impl Model {
    /// アーティファクト情報をパース
    pub fn artifacts(&self) -> Option<Vec<Artifact>> {
        self.artifacts_json.as_ref().and_then(|json| {
            serde_json::from_str(json).ok()
        })
    }
}

/// アーティファクト定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub filename: String,
    pub url: Option<String>,
    pub size_bytes: Option<u64>,
    pub sha256: Option<String>,
}
```

### ModelTag（モデルタグ）

```rust
/// モデルに紐付くタグ
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelTag {
    /// タグID（自動採番）
    pub id: i64,
    /// モデルID（外部キー）
    pub model_id: String,
    /// タグ名
    pub tag: String,
}
```

### ModelWithTags（タグ付きモデル）

```rust
/// タグを含むモデル情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelWithTags {
    #[serde(flatten)]
    pub model: Model,
    /// タグ一覧
    pub tags: Vec<String>,
}
```

### ModelFilter（検索フィルタ）

```rust
/// モデル検索フィルタ
#[derive(Debug, Clone, Default)]
pub struct ModelFilter {
    /// 名前による部分一致検索
    pub name: Option<String>,
    /// タグによるフィルタ（AND条件）
    pub tags: Option<Vec<String>>,
    /// ソースによるフィルタ
    pub source: Option<String>,
    /// ページネーション: オフセット
    pub offset: Option<i64>,
    /// ページネーション: 件数
    pub limit: Option<i64>,
}
```

## 検証ルール表

| フィールド | ルール | エラーメッセージ |
|-----------|--------|------------------|
| `name` | 空文字不可、255文字以内 | "Model name is required and must be <= 255 chars" |
| `name` | ユニーク制約 | "Model name already exists" |
| `source` | 定義済み値のみ | "Invalid model source" |
| `tags` | 各タグ50文字以内 | "Tag must be <= 50 chars" |
| `required_memory` | 0以上 | "Required memory must be non-negative" |

## SQLスキーマ

### models テーブル

```sql
CREATE TABLE models (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    size INTEGER,
    description TEXT,
    required_memory INTEGER,
    source TEXT CHECK(source IN ('hf_safetensors', 'hf_gguf', 'hf_onnx', 'predefined')),
    path TEXT,
    artifacts_json TEXT,
    repo_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_models_name ON models(name);
CREATE INDEX idx_models_source ON models(source);
```

### model_tags テーブル

```sql
CREATE TABLE model_tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    UNIQUE(model_id, tag)
);

CREATE INDEX idx_model_tags_tag ON model_tags(tag);
CREATE INDEX idx_model_tags_model_id ON model_tags(model_id);
```

## 関係図

```text
┌─────────────────────────────────────────────────────────────┐
│                        router.db                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ models                                               │    │
│  │  - id: TEXT (PK)                                     │    │
│  │  - name: TEXT (UNIQUE)                               │    │
│  │  - size: INTEGER                                     │    │
│  │  - source: TEXT                                      │    │
│  │  - artifacts_json: TEXT                              │    │
│  │  - created_at: TEXT                                  │    │
│  │  - updated_at: TEXT                                  │    │
│  └───────────────────────────┬─────────────────────────┘    │
│                              │ 1:N                           │
│                              ▼                               │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ model_tags                                           │    │
│  │  - id: INTEGER (PK)                                  │    │
│  │  - model_id: TEXT (FK → models.id)                   │    │
│  │  - tag: TEXT                                         │    │
│  │  - UNIQUE(model_id, tag)                             │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ users (既存)                                         │    │
│  │ api_keys (既存)                                      │    │
│  │ ...                                                  │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                    models.json.migrated                      │
│                    （移行後の元ファイル）                      │
└─────────────────────────────────────────────────────────────┘
```

## クエリパターン

### 全モデル一覧取得

```sql
SELECT m.*, GROUP_CONCAT(t.tag) as tags
FROM models m
LEFT JOIN model_tags t ON m.id = t.model_id
GROUP BY m.id
ORDER BY m.name;
```

### タグ検索（AND条件）

```sql
SELECT m.*, GROUP_CONCAT(t.tag) as tags
FROM models m
LEFT JOIN model_tags t ON m.id = t.model_id
WHERE m.id IN (
    SELECT model_id FROM model_tags WHERE tag IN ('vision', 'chat')
    GROUP BY model_id
    HAVING COUNT(DISTINCT tag) = 2
)
GROUP BY m.id;
```

### ソースフィルタリング

```sql
SELECT m.*, GROUP_CONCAT(t.tag) as tags
FROM models m
LEFT JOIN model_tags t ON m.id = t.model_id
WHERE m.source = 'hf_safetensors'
GROUP BY m.id;
```
