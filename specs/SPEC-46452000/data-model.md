# データモデル: LM Studioエンドポイントタイプ検出

**機能ID**: `SPEC-46452000` | **日付**: 2026-02-13

## 変更対象エンティティ

### 1. EndpointType enum

**ファイル**: `llmlb/src/types/endpoint.rs`

**変更**: `LmStudio` バリアント追加

- serde名: `lm_studio`
- `as_str()`: `"lm_studio"`
- `from_str()`: `"lm_studio" => Self::LmStudio`
- `supports_model_download()`: `false`
- `supports_model_metadata()`: `true`

### 2. ModelMetadata struct

**ファイル**: `llmlb/src/metadata/mod.rs`

**追加フィールド**（全タイプ共通、Option型）:

- `format: Option<String>` - モデルフォーマット（"gguf", "mlx"等）
- `supports_vision: Option<bool>` - ビジョン対応
- `supports_tool_use: Option<bool>` - ツール利用対応
- `quantization_bits: Option<f32>` - 量子化ビット数（bits_per_weight）

### 3. EndpointTypeDetection

**ファイル**: `llmlb/src/detection/mod.rs`

**変更**: 検出優先順位にLM Studioを追加

- Priority 1: xLLM
- Priority 2: Ollama
- **Priority 3: LM Studio（新規）**
- Priority 4: vLLM
- Priority 5: OpenAI-compatible
- Fallback: Unknown

### 4. メタデータ取得ルーティング

**ファイル**: `llmlb/src/metadata/mod.rs`

**変更**: `get_model_metadata()` にLmStudioブランチ追加

### 5. モデル同期

**ファイル**: `llmlb/src/sync/mod.rs`

**変更**: メタデータ取得対象にLmStudioを追加

## 新規ファイル

### `llmlb/src/detection/lm_studio.rs`

LM Studio検出ロジック。複合判定を実装:

1. `GET /api/v1/models` → publisher/arch/stateフィールド確認
2. Serverヘッダーに "lm-studio" or "LM Studio" 確認
3. `/v1/models` の owned_by に "lm-studio" 確認

### `llmlb/src/metadata/lm_studio.rs`

LM Studioメタデータ取得ロジック。
`GET /api/v1/models/{model}` からModelMetadataへのマッピング。

## DBスキーマ

endpoint_modelテーブルに既にmax_tokensカラムが存在。
ModelMetadataの新規フィールドはAPIレスポンスにのみ反映し、
DB永続化は既存のmax_tokensのみ（変更なし）。
