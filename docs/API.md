# API 補足: クラウドモデルプレフィックス

## 対象エンドポイント

- `POST /v1/chat/completions`
- `POST /v1/completions`
- `POST /v1/embeddings`

## モデル指定ルール

| プレフィックス | 転送先 | 例 |
| --- | --- | --- |
| `openai:` | OpenAI API (`OPENAI_BASE_URL`, 既定 `https://api.openai.com`) | `openai:gpt-4o` |
| `google:` | Google Generative Language API (`GOOGLE_API_BASE_URL`, 既定 `https://generativelanguage.googleapis.com/v1beta`) | `google:gemini-pro` |
| `anthropic:` (`ahtnorpic:` 可) | Anthropic API (`ANTHROPIC_API_BASE_URL`, 既定 `https://api.anthropic.com`) | `anthropic:claude-3-opus` |

プレフィックスは転送前に除去され、クラウド側にはプレフィックスなしのモデル名が送信されます。プレフィックスなしのモデルは従来どおりローカルLLMへルーティングされます。

## 必須環境変数

- `OPENAI_API_KEY`
- `GOOGLE_API_KEY`
- `ANTHROPIC_API_KEY`

任意: `OPENAI_BASE_URL`, `GOOGLE_API_BASE_URL`, `ANTHROPIC_API_BASE_URL`

## ストリーミング

`stream: true` を指定するとクラウドAPIのストリーミング(SSE/チャンク)をそのままパススルーします。

## メトリクス

- エンドポイント: `/api/metrics/cloud`（Prometheus text）
- 指標:
  - `cloud_requests_total{provider,status}`
  - `cloud_request_latency_seconds{provider}`
- 日次 export: `/api/metrics/cloud/export?format=json|csv&days=1..90`
  - provider（`openai` / `google` / `anthropic`）× UTC 日次の集計を返す
  - フィールド: `date, provider, request_count, success_count, error_count, avg_latency_ms, p95_latency_ms`
  - `days` が 1..90 の範囲外、`format` が json/csv 以外の場合は 400。90日を超えるデータは保持しない

## エラーハンドリングの方針

| ケース | ステータス | ボディ概要 |
| --- | --- | --- |
| APIキー未設定 | 401 Unauthorized | `error: "<PROVIDER>_API_KEY is required for ..."` |
| 不明/未実装プレフィックス | 400 Bad Request | `error: "unsupported cloud provider prefix"` |
| クラウド側4xx/5xx | クラウドと同じ | クラウドレスポンスをそのまま返却（JSON/SSEヘッダ維持） |

## トークン統計API

リクエストのトークン使用量を追跡・集計するAPIエンドポイント。

### エンドポイント

| メソッド | パス | 説明 |
| --- | --- | --- |
| GET | `/api/dashboard/stats/tokens` | 累計トークン統計 |
| GET | `/api/dashboard/stats/tokens/daily` | 日次トークン統計 |
| GET | `/api/dashboard/stats/tokens/monthly` | 月次トークン統計 |

### レスポンス形式

#### 累計統計 (`/api/dashboard/stats/tokens`)

```json
{
  "total_input_tokens": 12345,
  "total_output_tokens": 6789,
  "total_tokens": 19134,
  "request_count": 100
}
```

#### 日次統計 (`/api/dashboard/stats/tokens/daily?days=7`)

```json
[
  {
    "date": "2026-01-05",
    "total_input_tokens": 1000,
    "total_output_tokens": 500,
    "total_tokens": 1500,
    "request_count": 10
  }
]
```

#### 月次統計 (`/api/dashboard/stats/tokens/monthly?months=3`)

```json
[
  {
    "month": "2026-01",
    "total_input_tokens": 30000,
    "total_output_tokens": 15000,
    "total_tokens": 45000,
    "request_count": 300
  }
]
```

### トークン取得ロジック

1. **ランタイムレスポンスのusageフィールド**（優先）: OpenAI互換APIの`usage`フィールドから取得
2. **tiktoken推定**（フォールバック）: usageがない場合はtiktoken-rsでトークン数を推定
3. **ストリーミング**: チャンクごとに累積し、最終チャンクのusageを使用

## 互換APIのフィールド透過

llmlb は上流 API の新フィールドに追従するため、リクエストを型付けせずに転送するルート
（透過ルート）と、ペイロードを組み立て直すルート（再構築ルート）を区別している。
再構築ルートは、明示的に転記していないフィールドを上流へ送らない。

行番号は Issue #775 の対応時点のもの。コードで確認できた事実のみを記載している。

### 透過ルート（未知フィールドは上流へ到達する）

| ルート | llmlb が書き換えるもの |
|---|---|
| `POST /v1/responses` | `model` のみ |
| `POST /v1/chat/completions` | `model`、`stream: true` 時の `stream_options.include_usage`（未指定時のみ追加） |
| `POST /v1/completions` | 同上 |
| `POST /v1/embeddings` | 同上 |

この性質は契約テスト `llmlb/tests/contract/unknown_field_passthrough_test.rs` が固定している。
上流スタブが受信したボディを記録し、合成キー `__llmlb_unknown_probe` と実在フィールドが
同じ値で届いたことを表明する。これらのルートに allow-list や型付き struct を導入すると
テストが失敗する。

### 再構築ルート（破棄されるフィールド）

#### `POST /v1/messages`（ローカルエンドポイント向け）

Anthropic 形式を OpenAI 互換形式へ変換する際、新しいオブジェクトを組み立てる
（`llmlb/src/api/anthropic/convert.rs:229`）。転記されるトップレベルフィールドは
`model` / `messages` / `system` / `max_tokens` / `stream` / `temperature` / `top_p` /
`top_k` / `metadata` / `stop_sequences` / `tools` / `tool_choice` のみ。

| 破棄・拒否されるもの | 挙動 | 根拠 |
|---|---|---|
| `thinking` | 破棄。変換関数が参照しない | `llmlb/src/api/anthropic/convert.rs:229-269` |
| `service_tier` など、上記以外のトップレベルフィールドすべて | 破棄。変換関数が参照しない | `llmlb/src/api/anthropic/convert.rs:229-269` |
| `max_tokens` の省略 | 400 で拒否（必須扱い） | `llmlb/src/api/anthropic/convert.rs:49-58` |
| 数値でない `temperature` / `top_p` | 破棄（エラーにしない） | `llmlb/src/api/anthropic/convert.rs:235-240` |
| `system` の `text` 以外のブロック | 400 で拒否 | `llmlb/src/api/anthropic/convert.rs:78`, `:402-411` |
| `system` の `text` ブロックが持つ `text` 以外のメンバー（`cache_control` 等） | 破棄。`text` を連結した平文だけが残る | `llmlb/src/api/anthropic/convert.rs:412-419` |
| `messages[].content` の `text` 以外のブロック（`image` 等。ツールブロックを含まないメッセージ） | 400 で拒否 | `llmlb/src/api/anthropic/convert.rs:221`, `:402-411` |
| `tool_use` を含む assistant メッセージ内の `text` / `tool_use` 以外のブロック | 破棄 | `llmlb/src/api/anthropic/convert.rs:148` |
| `tool_result` を含む user メッセージ内の `text` / `tool_result` 以外のブロック | 破棄 | `llmlb/src/api/anthropic/convert.rs:207` |
| `tools[]` の `name` / `description` / `input_schema` 以外のメンバー | 破棄 | `llmlb/src/api/anthropic/convert.rs:311-318` |
| `tools[].input_schema` の `type` / `properties` / `required` 以外のメンバー | 破棄 | `llmlb/src/api/anthropic/convert.rs:300-309` |
| `tool_choice` の `type` / `name` 以外のメンバー | 破棄 | `llmlb/src/api/anthropic/convert.rs:325-346` |
| `tool_choice.type` が `auto` / `any` / `tool` 以外 | 400 で拒否 | `llmlb/src/api/anthropic/convert.rs:347-351` |

`anthropic:` プレフィックスでクラウドへ転送する経路は変換を通らない
（`llmlb/src/api/anthropic.rs:86`）ため、この表の対象外。

#### `POST /v1/audio/speech`

型付き struct で受け、それをシリアライズし直して転送する
（`llmlb/src/api/audio.rs:399`, `:454`）。

| 破棄・拒否されるもの | 挙動 | 根拠 |
|---|---|---|
| `model` / `input` / `voice` / `response_format` / `speed` 以外のフィールド（`instructions`、`stream_format` 等） | 破棄 | `llmlb/src/common/protocol.rs:252-266` |
| `response_format` が `wav` / `mp3` / `flac` / `ogg` / `opus` 以外 | デシリアライズ失敗で拒否 | `llmlb/src/types/media.rs:10-22` |
| 省略された `voice` / `response_format` / `speed` | 既定値（`nova` / `mp3` / `1.0`）が補われて送られる | `llmlb/src/common/protocol.rs:258-265` |

#### `POST /v1/images/generations`

型付き struct で受け、それをシリアライズし直して転送する
（`llmlb/src/api/images.rs:199`, `:251`）。

| 破棄・拒否されるもの | 挙動 | 根拠 |
|---|---|---|
| `model` / `prompt` / `n` / `size` / `quality` / `style` / `response_format` / `negative_prompt` / `seed` / `steps` 以外のフィールド | 破棄 | `llmlb/src/common/protocol.rs:278-307` |
| `size` が `256x256` / `512x512` / `1024x1024` / `1792x1024` / `1024x1792` 以外 | デシリアライズ失敗で拒否 | `llmlb/src/types/media.rs:26-43` |
| `quality` が `standard` / `hd` 以外 | デシリアライズ失敗で拒否 | `llmlb/src/types/media.rs:48-54` |
| `style` が `vivid` / `natural` 以外 | デシリアライズ失敗で拒否 | `llmlb/src/types/media.rs:59-65` |
| `response_format` が `url` / `b64_json` 以外 | デシリアライズ失敗で拒否 | `llmlb/src/types/media.rs:70-77` |
| `n` が 1〜10 の範囲外 | 400 で拒否 | `llmlb/src/api/images.rs:215-217` |
| 省略された `n` / `size` / `quality` / `style` / `response_format` | 既定値が補われて送られる | `llmlb/src/common/protocol.rs:284-297` |

#### multipart ルート

multipart をフィールド単位で読み取り、新しいフォームを組み立てる。

| ルート | 転記されるフィールド | 破棄されるもの | 根拠 |
|---|---|---|---|
| `POST /v1/audio/transcriptions` | `file` / `model` / `language` / `response_format` | それ以外のフィールドすべて（読み取り時に無視）。`file` パートの Content-Type は `audio/wav` に固定 | `llmlb/src/api/audio.rs:243-286`, `:327-343` |
| `POST /v1/images/edits` | `image` / `mask` / `prompt` / `model` / `n` / `size` / `response_format` | それ以外のフィールドすべて。`image` / `mask` パートの Content-Type は `image/png` に固定 | `llmlb/src/api/images.rs:337-410`, `:448-479` |
| `POST /v1/images/variations` | `image` / `model` / `n` / `size` / `response_format` | それ以外のフィールドすべて。`image` パートの Content-Type は `image/png` に固定 | `llmlb/src/api/images.rs:564-616`, `:649-669` |
