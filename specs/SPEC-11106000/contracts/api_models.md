# 契約: Models API 拡張 (SPEC-11106000)

## GET /v0/models/available
- **Purpose**: HF GGUF カタログを返す。
- **Query**: `search`, `limit`, `offset`, `source=hf` (デフォルト hf)。
- **Response** 200:
```json
{
  "models": [
    {
      "name": "hf/TheBloke/Llama-2-7B-GGUF/llama-2-7b.Q4_K_M.gguf",
      "display_name": "Llama-2-7B Q4_K_M (TheBloke)",
      "source": "hf_gguf",
      "size_bytes": 5242880000,
      "download_url": "https://huggingface.co/.../llama-2-7b.Q4_K_M.gguf",
      "repo": "TheBloke/Llama-2-7B-GGUF",
      "filename": "llama-2-7b.Q4_K_M.gguf",
      "last_modified": "2025-11-30T12:00:00Z",
      "tags": ["gguf","q4_k_m"],
      "status": "available"
    }
  ],
  "source": "hf",
  "cached": false,
  "pagination": { "limit": 20, "offset": 0, "total": 123 }
}
```

## POST /v0/models/register
- **Purpose**: HF GGUF を対応モデルとして登録。
- **Body**:
```json
{
  "repo": "TheBloke/Llama-2-7B-GGUF",
  "filename": "llama-2-7b.Q4_K_M.gguf",
  "display_name": "Llama-2-7B Q4_K_M (TheBloke)"
}
```
- **Response** 201:
```json
{ "name": "hf/TheBloke/Llama-2-7B-GGUF/llama-2-7b.Q4_K_M.gguf", "status": "registered" }
```
- **Errors**: 400 無効名/URL欠損, 409 重複, 424 HFから取得不可。

## GET /v1/models

- 対応モデルに HF 登録分も含めて返す（idのみ。displayやsourceは拡張フィールドとしてオプション）。
- ノードはこの一覧を参照し、`path` が参照できない場合は `GET /v0/models/blob/:model_name` でモデルを取得する（ルーターからのpush配布は行わない）。

---

## CLI コマンド（廃止）

**廃止日**: 2025-12-10

CLIコマンドは廃止されました。以下のAPIを直接使用してください：

- モデル一覧: `GET /v0/models/available`
- モデル登録: `POST /v0/models/register`
- ノード同期（一覧）: `GET /v1/models`
- ノード同期（ファイル）: `GET /v0/models/blob/:model_name`
