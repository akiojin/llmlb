# API Client Examples

Sample calls for the OpenAI-compatible router with cloud prefixes.

## Default text model

A platform administrator can open **Dashboard → Settings → Inference**, choose
**Default text model**, and click **Save**. Choose **Not set (model required)**
and save to clear it. The setting is shared by every user and organization on
this llmlb instance. Offline known models can be saved; saving does not load or
download a model. If a saved model disappears from the catalog, the dashboard
keeps its ID visible so an administrator can replace or clear it.

The setting is stored in the existing SQLite database. Keep the database at
`LLMLB_DATABASE_URL` on persistent storage, including when running in a container.
No new environment variable or restart is needed. A successful save takes effect
for requests that read the default after the database commit. Already queued or
streaming requests keep the model chosen when they arrived.

The dedicated management API requires an authenticated platform administrator:

```bash
# ADMIN_JWT is a dashboard-session JWT, not an inference API key.
curl http://localhost:32768/api/dashboard/inference-defaults \
  -H "Authorization: Bearer $ADMIN_JWT"

curl -X PUT http://localhost:32768/api/dashboard/inference-defaults \
  -H "Authorization: Bearer $ADMIN_JWT" \
  -H "Content-Type: application/json" \
  -d '{"default_text_model":"your-registered-text-model"}'

# Clear the default. GET returns the same nullable wire shape.
curl -X PUT http://localhost:32768/api/dashboard/inference-defaults \
  -H "Authorization: Bearer $ADMIN_JWT" \
  -H "Content-Type: application/json" \
  -d '{"default_text_model":null}'
```

Cookie-authenticated PUT requests also require the existing `X-CSRF-Token` and
same-origin checks. Viewer, organization manager, and inference API-key
credentials cannot manage this setting. The generic settings API cannot read
or write the reserved `inference.defaults` key.

Model omission is an **llmlb HTTP JSON extension**. OpenAI and Anthropic SDK
request types still require `model`; use an explicit model with those types, or
send HTTP JSON directly when omission is needed. Only an absent `model` key uses
the default. Explicit values, including `null`, an empty string, or an invalid
type, keep their existing validation behavior and never fall back to it.

After setting a compatible default, a direct HTTP request can omit the key:

```bash
curl -X POST http://localhost:32768/v1/chat/completions \
  -H "Authorization: Bearer $LLMLB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"messages":[{"role":"user","content":"Hello"}]}'

# An explicit model always takes precedence.
curl -X POST http://localhost:32768/v1/chat/completions \
  -H "Authorization: Bearer $LLMLB_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"another-text-model","messages":[{"role":"user","content":"Hello"}]}'
```

| API | Behavior when `model` is omitted |
| --- | --- |
| `/v1/chat/completions`, dashboard LB Playground | Use the global default; chat and image-input capabilities must match |
| `/v1/completions` | Use a local text-generation default; cloud defaults are incompatible |
| `/v1/responses` | Use a local default on a Responses-capable endpoint; cloud defaults are incompatible |
| `/v1/messages` | Use a local chat-capable or native `anthropic:` default; other required fields and headers remain mandatory |
| `/v1/embeddings` | Keep its existing embedding environment-variable and built-in defaults |
| `/v1/images/generations`, `/v1/audio/speech`, `/v1/audio/transcriptions` | Keep their existing required-model validation |
| `/v1/images/edits`, `/v1/images/variations` | Keep their existing `stable-diffusion-xl` multipart default |

A saved default does not guarantee support for every API. An incompatible
default returns HTTP 400, an unknown model keeps the existing not-found error,
and an offline model keeps the existing unavailable error. llmlb does not choose
the first available model or retry another model. Without a default, the text
generation APIs continue to require an explicit model.

## curl
```bash
curl -X POST http://localhost:32768/v1/responses \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openai:gpt-4o",
    "input": "Hello"
  }'
```

## Python (requests)
```python
import requests

payload = {
    "model": "google:gemini-1.5-pro",
    "input": "Say hi in JSON",
}
resp = requests.post("http://localhost:32768/v1/responses", json=payload)
resp.raise_for_status()
print(resp.json())
```

## Node.js (fetch)
```javascript
const body = {
  model: "anthropic:claude-3-opus",
  input: "Give me three bullets",
};

const res = await fetch("http://localhost:32768/v1/responses", {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify(body),
});

const data = await res.json();
console.log(data);
```

## Endpoint timeout guidance

When you register a new endpoint, llmlb sets `inference_timeout_secs` from the
detected `endpoint_type`.

- `xllm`, `ollama`, `lm_studio`: `600`
- `vllm`, `llamacpp`, `openai_compatible`: `120`
- `sglang`, `lmdeploy`, `xinference`, `lemonade`, `tgi`, `koboldcpp`: `120`

Use `600` seconds or more for large local models that may spend several
minutes loading weights before the first response. Existing endpoints are not
migrated automatically, so update the timeout from the dashboard if an older
local runtime still uses `120`.

## Claude Code から接続する

llmlb は Anthropic 互換の `/v1/messages` を提供するため、
[Claude Code](https://docs.anthropic.com/en/docs/claude-code) のバックエンドとして利用できます。

以下の 3 つの環境変数を設定してから `claude` を起動してください。

```bash
# PowerShell / bash / zsh 共通の例
export ANTHROPIC_BASE_URL="http://localhost:32768"
export ANTHROPIC_API_KEY="<llmlb で発行した API キー>"   # 例: sk_debug（デバッグビルドのみ）
export ANTHROPIC_MODEL="<エンドポイントに登録済みのモデル>"  # 例: openai/gpt-oss-20b
```

| 変数 | 役割 |
|------|------|
| `ANTHROPIC_BASE_URL` | Claude Code がリクエストを送る先。llmlb のベース URL（スキーム・ホスト・ポートのみ） |
| `ANTHROPIC_API_KEY` | llmlb のダッシュボードで発行した API キー。デバッグビルドでは `sk_debug` が使える |
| `ANTHROPIC_MODEL` | 使用するモデル。canonical 名（HuggingFace レポ ID 形式）を推奨 |

### モデル名の扱い

`/v1/messages` は受信したモデル名をそのまま upstream に転送せず、
`resolve_canonical_any` + `rewrite_payload_model_for_endpoint` によって
エンドポイントが広告するエイリアス名へ自動変換します。

例: Ollama に `gpt-oss:20b` として登録されたモデルに対し
`ANTHROPIC_MODEL=openai/gpt-oss-20b` を指定した場合、
llmlb は upstream に `gpt-oss:20b` として転送します。

この仕組みにより、クライアント（Claude Code 等）は
エンドポイント種別（Ollama / LM Studio / vLLM / xLLM）ごとの
命名差異を意識せずに canonical 名のみで利用できます。

### 認証競合警告が出る場合

Claude Code 起動時に以下の警告が出ることがあります。

```text
⚠ Auth conflict: Both a token (claude.ai) and an API key (ANTHROPIC_API_KEY) are set.
```

llmlb を使う場合は `ANTHROPIC_API_KEY` 側が正です。以下のいずれかで解消してください。

- `claude /logout` で claude.ai のトークンを破棄する
- または、ログイン時に API キー承認のプロンプトで **No** と答え、セッションを `ANTHROPIC_API_KEY` 側に固定する

### 動作確認（curl）

Claude Code を介さず、直接 llmlb に Anthropic 形式で疎通確認する例です。

```bash
curl -sS -X POST "$ANTHROPIC_BASE_URL/v1/messages" \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d "{\"model\":\"$ANTHROPIC_MODEL\",\"max_tokens\":32,\"messages\":[{\"role\":\"user\",\"content\":\"ping\"}]}"
```

`200 OK` と `"type":"message"` を含む JSON が返れば接続成功です。
