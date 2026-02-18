# 技術リサーチ: LM Studioエンドポイントタイプ検出

**機能ID**: `SPEC-af1ec86d` | **日付**: 2026-02-13

## LM Studio API調査結果

### バージョン体系

- 0.2.x系: 旧API、固有エンドポイント限定的
- 0.3.x系: `/lmstudio/` プレフィックスの固有API導入
- 0.4.0: ネイティブREST API v1 (`/api/v1/*`) 正式リリース、ヘッドレスデーモン（llmster）導入
- **ターゲット**: 0.4.0以降（最新安定版のみ）

### API体系（3系統が併存）

| API群 | パスプレフィックス | 用途 |
|---|---|---|
| ネイティブREST v1 | `/api/v1/*` | LM Studio固有機能（推奨） |
| OpenAI互換 | `/v1/*` | OpenAI SDKとの互換 |
| レガシーREST v0 | `/api/v0/*` | 旧API（非推奨） |

### 検出に利用可能なエンドポイント

#### Primary: `GET /api/v1/models`（最も信頼性が高い）

LM Studio固有のパスであり、他のサーバー（vLLM, Ollama等）は404を返す。

**レスポンス形式**:

```json
{
  "object": "list",
  "data": [
    {
      "id": "meta-llama-3.1-8b-instruct",
      "object": "model",
      "type": "llm",
      "publisher": "lmstudio-community",
      "arch": "llama",
      "compatibility_type": "gguf",
      "quantization": "Q4_K_M",
      "state": "not-loaded",
      "max_context_length": 131072
    }
  ]
}
```

**固有フィールド**（他のOpenAI互換APIには存在しない）:

- `type`: "llm" | "embedding" | "vlm"
- `publisher`: モデルの公開者
- `arch`: モデルアーキテクチャ
- `compatibility_type`: "gguf" | "mlx"
- `quantization`: 量子化タイプ
- `state`: "loaded" | "not-loaded"
- `max_context_length`: 最大コンテキスト長

#### Fallback 1: Serverヘッダー

公式ドキュメントに記載なし。バージョンにより"LM Studio"や"lm-studio"等の
表記揺れの可能性あり。実機検証で確認が必要。

#### Fallback 2: `/v1/models` の `owned_by` フィールド

バージョンにより"lm-studio"が含まれる場合がある。

### メタデータ取得

`GET /api/v1/models` から以下が取得可能:

| LM Studioフィールド | ModelMetadataマッピング |
|---|---|
| `max_context_length` | `context_length` |
| `size_bytes`（v1詳細） | `size_bytes` |
| `quantization` / `quantization.name` | `quantization` |
| `arch` | `family` |
| `params_string` | `parameter_size` |
| `compatibility_type` | `format`（新規） |
| `capabilities.vision` | `supports_vision`（新規） |
| `capabilities.trained_for_tool_use` | `supports_tool_use`（新規） |
| `quantization.bits_per_weight` | `quantization_bits`（新規） |

### 個別モデル情報

`GET /api/v1/models/{model}` で個別モデルの詳細情報が取得可能。
一覧APIと同等のフィールドを返す。

### 認証

- デフォルト: 認証不要
- オプション: `Authorization: Bearer $LM_API_TOKEN`
- 既存のエンドポイント api_key をそのまま利用可能

### 既存パターンとの整合性

| 項目 | xLLM | Ollama | vLLM | LM Studio |
|---|---|---|---|---|
| 検出パス | `/api/system` | `/api/tags` | `/v1/models` | `/api/v1/models` |
| 検出フィールド | `xllm_version` | `models`配列 | Serverヘッダー | `publisher`/`arch`/`state` |
| メタデータAPI | `/v0/system` | `/api/show` | なし | `/api/v1/models` |
| supports_metadata | true | true | false | true |
| supports_download | true | false | false | false |
