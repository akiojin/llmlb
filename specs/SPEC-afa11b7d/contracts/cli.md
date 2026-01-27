# CLI契約: safetensors量子化対応

**機能ID**: `SPEC-afa11b7d`
**作成日**: 2026-01-27

## 目的

CLIで量子化の指定および結果を確認できることを保証する。

## 期待される振る舞い

### 量子化指定

- safetensorsモデルに対して次の量子化方式を指定できること
  - `kv_int8`
  - `kv_fp8`
- 未対応の量子化方式指定は明確なエラーを返すこと

### 表示

- `xllm show <MODEL>` で量子化情報が表示されること（適用時）
  - `Details` セクションの `Quantization` 行に表示される
  - 値は `kv_int8` / `kv_fp8` / `Q4_K_M` 等の量子化トークン
- `xllm list` で量子化が判別可能であること
  - 既存の列構成は維持する（NAME/ID/SIZE/MODIFIED）
  - 量子化が判明している場合、NAME列を`name:<quant>`で表示する

### エラー

- 未対応方式: `Unsupported quantization '<token>'` を含むメッセージ
  - safetensorsの場合は `kv_int8, kv_fp8` を案内に含める
- 不正な指定形式: `Invalid model name (invalid quantization format)` を含むメッセージ
