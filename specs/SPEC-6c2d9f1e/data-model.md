# データモデル: モデル登録キャッシュとマルチモーダルI/O

## エンティティ定義

### RuntimeType

ノードがサポートする推論ランタイム。

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeType {
    LlamaCpp,
    Whisper,
    StableDiffusion,
    OnnxRuntime,
}
```

### NodeRegistration

ノード登録時のペイロード。

```rust
pub struct NodeRegistration {
    pub node_id: Uuid,
    pub node_ip: String,
    pub port: u16,
    pub supported_runtimes: Vec<RuntimeType>,
    pub loaded_models: LoadedModels,
    pub gpu_info: GpuInfo,
}

pub struct LoadedModels {
    pub llm: Vec<String>,
    pub asr: Vec<String>,
    pub tts: Vec<String>,
    pub image_gen: Vec<String>,
}
```

### NodeHeartbeat

ハートビートペイロード。

```rust
pub struct NodeHeartbeat {
    pub node_id: Uuid,
    pub supported_runtimes: Vec<RuntimeType>,
    pub loaded_models: LoadedModels,
    pub status: NodeStatus,
}
```

### CacheStatus

キャッシュ状態の判定結果。

```rust
pub enum CacheStatus {
    Valid,      // サイズ > 0、再利用可能
    Invalid,    // サイズ = 0、再ダウンロード必要
    NotFound,   // ファイルなし、ダウンロード必要
}
```

## APIルーティングマッピング

### RuntimeTypeによるルーティング

```rust
fn get_required_runtime(endpoint: &str) -> RuntimeType {
    match endpoint {
        "/v1/chat/completions" | "/v1/completions" => RuntimeType::LlamaCpp,
        "/v1/audio/transcriptions" => RuntimeType::Whisper,
        "/v1/audio/speech" => RuntimeType::OnnxRuntime,
        "/v1/images/generations" | "/v1/images/edits" | "/v1/images/variations"
            => RuntimeType::StableDiffusion,
        _ => RuntimeType::LlamaCpp, // デフォルト
    }
}
```

### ノード選択

```rust
fn select_node(
    nodes: &[NodeInfo],
    required_runtime: RuntimeType,
    model_name: &str,
) -> Option<&NodeInfo> {
    nodes.iter()
        .filter(|n| n.supported_runtimes.contains(&required_runtime))
        .filter(|n| n.has_model_loaded(model_name))
        .min_by_key(|n| n.current_load)
}
```

## エラーモデル

### ModelNotReadyError

```rust
pub struct ModelNotReadyError {
    pub model: String,
    pub reason: NotReadyReason,
}

pub enum NotReadyReason {
    NoSupportingNode,
    CacheInvalid,
    Downloading,
}
```

### エラーレスポンスJSON

```json
{
  "error": {
    "message": "Model 'llama-3.1-8b' is not ready: no supporting node available",
    "type": "service_unavailable",
    "code": "model_not_ready"
  }
}
```

## 削除操作のデータフロー

### DELETE /v1/models/:name

```text
Request
  |
  v
Router: モデル検索
  |
  v
Router: 登録情報削除
  |
  v
Router: 全ノードへ削除通知 (並列)
  |
  v
Node: ローカルキャッシュ削除
  |
  v
Response: 200 OK / 404 Not Found
```

## シリアライゼーション

### JSON形式

```json
{
  "node_id": "550e8400-e29b-41d4-a716-446655440000",
  "supported_runtimes": ["llama_cpp", "whisper"],
  "loaded_models": {
    "llm": ["llama-3.1-8b"],
    "asr": ["whisper-large-v3"],
    "tts": [],
    "image_gen": []
  }
}
```
