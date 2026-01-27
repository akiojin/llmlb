# データモデル: safetensors量子化対応

**機能ID**: `SPEC-afa11b7d`
**作成日**: 2026-01-27

## 目的

safetensorsモデルの量子化設定を一貫した形で扱い、指定・適用・可視化・エラー処理に必要な最小限のメタデータを定義する。

## エンティティ

### QuantizationRequest

量子化指定の正規化表現。

- **token**: 量子化トークン
  - safetensors: `kv_int8` / `kv_fp8`
  - GGUF: 既存の`Q4_K_M`等（本SPECの対象外）
- **target**: 量子化対象（safetensorsでは常に`kv_cache`）
- **source**: 指定元（現時点ではモデル名サフィックス）
- **strict**: 未対応時にフォールバックしない（常にtrue）

### QuantizationStatus

量子化の適用結果を示す状態（エラー整形や表示のための概念）。

- **disabled**: 量子化指定なし
- **applied**: 指定方式で適用済み
- **unsupported**: 指定方式が未対応
- **invalid**: 指定形式が不正

### ModelMetadata 拡張

既存のModelDescriptorのmetadataに以下を追加する想定。

- **quantization_request**: リクエストされた量子化トークン（例: `kv_int8`）
- **quantization**: 実際に適用された量子化トークン
  - safetensorsでは、妥当な指定なら`quantization_request`と同値
- **quantization_target**: 量子化対象（safetensorsは`kv_cache`）
- **quantization_backend**: 適用先エンジン（例: `safetensors_cpp`）
- **quantization_error**: 失敗時の簡潔な理由（将来拡張用）

## ルール

- safetensorsの量子化指定は`kv_*`トークンに限定する
  - 許可: `kv_int8`, `kv_fp8`
  - それ以外は`unsupported`として扱う
- `kv_*`はsafetensors専用の予約トークンとして扱う
- 量子化指定なしの場合は`disabled`として扱う
- 未対応方式は`unsupported`で明示し、暗黙のフォールバックを禁止する
