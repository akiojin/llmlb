# データモデル: gpt-oss-20b safetensors 実行

## エンティティ定義

### ModelDescriptor

モデル記述子。登録時のメタデータと実行情報を保持。

```rust
pub struct ModelDescriptor {
    /// モデル識別子
    pub id: String,
    /// HuggingFace リポジトリ
    pub repo_id: String,
    /// 登録形式（常に safetensors）
    pub format: ModelFormat,
    /// config.json から抽出した構成
    pub config: ModelConfig,
    /// 実行キャッシュ（公式GPU最適化アーティファクト）
    pub execution_cache: Option<ExecutionCache>,
}
```

### ModelConfig

config.json から抽出したモデル構成。

```rust
pub struct ModelConfig {
    pub hidden_size: u32,
    pub num_hidden_layers: u32,
    pub num_attention_heads: u32,
    pub vocab_size: u32,
    pub max_position_embeddings: u32,
}
```

### SafetensorsManifest

safetensors マニフェスト。

```rust
pub struct SafetensorsManifest {
    /// シャーディングの場合の index ファイル
    pub index_file: Option<String>,
    /// shard ファイル一覧
    pub shards: Vec<String>,
    /// 総サイズ（バイト）
    pub total_size: u64,
}
```

### ExecutionCache

公式GPU最適化アーティファクトのキャッシュ情報。

```rust
pub struct ExecutionCache {
    /// 取得元リポジトリ
    pub source_repo: String,
    /// ファイルパス
    pub file_path: String,
    /// 対応バックエンド
    pub backend: GpuBackend,
}

pub enum GpuBackend {
    Metal,
    DirectML,
    Cuda,
}
```

## API契約

### モデル登録

```text
POST /v1/models/register
Content-Type: application/json

{
  "name": "gpt-oss-20b",
  "source": "openai/gpt-oss-20b",
  "format": "safetensors"
}
```

### 登録成功レスポンス

```json
{
  "id": "gpt-oss-20b",
  "object": "model",
  "created": 1700000000,
  "owned_by": "openai"
}
```

### 登録失敗レスポンス（メタデータ不足）

```json
{
  "error": {
    "message": "Required file missing: config.json",
    "type": "invalid_request_error",
    "code": "missing_required_file"
  }
}
```

## データベーススキーマ

### models テーブル拡張

```sql
ALTER TABLE models ADD COLUMN safetensors_manifest TEXT;
ALTER TABLE models ADD COLUMN execution_cache TEXT;
```

### JSON 格納形式

```json
{
  "safetensors_manifest": {
    "index_file": "model.safetensors.index.json",
    "shards": ["model-00001.safetensors", "model-00002.safetensors"],
    "total_size": 40000000000
  },
  "execution_cache": {
    "source_repo": "openai/gpt-oss-20b-metal",
    "file_path": "metal/model.bin",
    "backend": "metal"
  }
}
```
