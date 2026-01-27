# データモデル: safetensors量子化対応

**機能ID**: `SPEC-afa11b7d`
**作成日**: 2026-01-27

## 目的

safetensorsモデルの量子化設定を一貫した形で扱い、指定・適用・可視化・エラー処理に必要な最小限のメタデータを定義する。

## エンティティ

### QuantizationRequest

量子化指定の正規化表現。

- **id**: 量子化方式識別子（例: int8, fp8, mxfp4 など）
- **source**: 指定元（cli/api/config/default）
- **strict**: 未対応時にフォールバックを許可しないフラグ

### QuantizationStatus

量子化の適用結果を示す状態。

- **disabled**: 量子化指定なし
- **applied**: 指定方式で適用済み
- **unsupported**: 指定方式が未対応
- **invalid**: 指定形式が不正

### ModelMetadata 拡張

既存のModelDescriptorのmetadataに以下を追加する想定。

- **quantization**: 実際に適用された量子化方式（safetensors向け）
- **quantization_request**: リクエストされた量子化方式
- **quantization_status**: 上記QuantizationStatus
- **quantization_backend**: 適用先エンジン（例: safetensors_cpp）
- **quantization_error**: 失敗時の簡潔な理由

## ルール

- GGUFの量子化表記と混同しない（safetensors専用の扱いを明示）
- 量子化指定なしの場合は`quantization_status=disabled`とする
- 未対応方式は`unsupported`で明示し、暗黙のフォールバックを禁止する
