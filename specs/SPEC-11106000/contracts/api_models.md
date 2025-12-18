# 契約: Models API（HF登録 / ONNX優先）(SPEC-11106000)

## POST /v0/models/register
- **Purpose**: Hugging Face のモデル（リポジトリ/ファイル）を登録し、必要に応じてONNX変換（export）をキューする。
- **Notes**:
  - `.gguf` はサポート外（400）。
  - `filename` 省略時は、リポジトリ内の `.onnx` を探索し、無ければ Transformers→ONNX export を試行する。
  - `filename` が `.onnx` 以外の場合も export 経路になる（`filename` は無視される）。
  - `trust_remote_code` は省略可能（デフォルト false）。インストール時に同意済みで `LLM_ROUTER_TRUST_REMOTE_CODE_DEFAULT=1` の場合、export 経路では自動で有効化される。
- **Body**:
```json
{
  "repo": "sshleifer/tiny-gpt2",
  "filename": "model.onnx",
  "display_name": "Tiny GPT-2 (optional)",
  "chat_template": "optional",
  "trust_remote_code": false
}
```
- **Response** 201:
```json
{
  "name": "sshleifer/tiny-gpt2",
  "status": "registered",
  "size_bytes": 123456,
  "required_memory_bytes": 185184,
  "warnings": []
}
```
- **Errors**:
  - 400: 入力不正（GGUF/不正なrepo/不正なfilenameなど）
  - 409: 重複登録
  - 502/504: HFアクセス/タイムアウト

## GET /v0/models
- **Purpose**: 登録モデル一覧（進捗/失敗理由を含む）を返す。
- **Auth**: APIキーまたはノードトークン（開発環境では `Bearer sk_debug`）。
- **Response** 200: `RegisteredModelView[]`
  - `lifecycle_status`: `pending` / `caching` / `registered` / `error`
  - `download_progress.error`: 失敗理由（`error` のとき）

## DELETE /v0/models/:model_name
- **Purpose**: 登録モデルを削除する（キュー/変換中ならキャンセルも含む）。
- **Response**: 204

## GET /v1/models
- **Purpose**: OpenAI互換のモデル一覧を返す。
- **Notes**: 実体が存在するモデルのみ含む（ダウンロード/変換が未完了のものは含めない）。
