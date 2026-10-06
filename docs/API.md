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

## 運用通知（メール）

全登録エンドポイントの状態一覧を 1 日 1 回、メールで管理者へ送る（日次ダイジェスト）。
エンドポイントが Offline に到達したときは、同じ宛先へ即時通知を送る（`EndpointStatusChanged` イベントを購読する）。
ただし、このイベントをヘルスチェックの状態遷移から発行する変更（Issue #692 / PR #728）が未着地の間は、
即時通知は送られない。

### 設定の置き場所

| 種別 | 置き場所 | 項目 |
|------|----------|------|
| 秘密情報 | 環境変数 | `LLMLB_SMTP_USERNAME` / `LLMLB_SMTP_PASSWORD` |
| 非秘密設定 | `settings` テーブル | 下表の `notifications.*` |
| 宛先 | `users.email` | email を設定済みの管理者（admin） |

| キー | 既定値 | 説明 |
|------|--------|------|
| `notifications.enabled` | `false` | 通知の有効／無効 |
| `notifications.smtp_host` | （未設定） | SMTP ホスト名または IP アドレス |
| `notifications.smtp_port` | `587` | SMTP ポート。`465` は接続時から TLS、それ以外は STARTTLS |
| `notifications.smtp_from` | （未設定） | 差出人アドレス |
| `notifications.daily_digest_time` | `09:00` | 送信時刻（サーバーのローカル時刻、`HH:MM`） |
| `notifications.language` | `ja` | メール本文の言語（`ja` / `en`） |
| `notifications.daily_digest_last_sent_date` | （未設定） | 最終送信日。スケジューラが更新する |
| `notifications.daily_digest_last_error` | （未設定） | 直近の送信失敗（日時と理由）。スケジューラが更新し、成功すると空に戻る |

`users.email` は通知先であり、ログイン識別子ではない。`POST /api/users` と `PUT /api/users/:id` の
`email` で設定し（`PUT` の空文字で解除）、ユーザー一覧のレスポンスに含まれる。
複数のユーザーが同じアドレスを共有でき、その場合も送信は 1 通にまとまる。

### 設定 API

`GET /api/dashboard/notifications` と `PUT /api/dashboard/notifications`（どちらも JWT の admin のみ）。
`PUT` のボディは `settings` と同じ形で、形式が不正な値は 400 で拒否する。
`notifications.*` はこの API だけが扱う。汎用の `/api/dashboard/settings/{key}` からは読み書きできない（403）。

```json
{
  "settings": {
    "enabled": true,
    "smtp_host": "smtp.example.com",
    "smtp_port": 587,
    "smtp_from": "llmlb@example.com",
    "daily_digest_time": "09:00",
    "language": "ja"
  },
  "status": { "state": "active", "reason": null },
  "credentials_configured": true,
  "recipients": [{ "username": "admin", "email": "ops@example.com" }],
  "last_digest_sent_date": "2026-10-01",
  "last_digest_error": null
}
```

`status.state` は次のいずれか。`active` 以外では `reason` に送信できない理由が入る。

| state | 意味 |
|-------|------|
| `active` | 送信できる |
| `disabled` | 管理者が無効にしている |
| `unavailable` | 有効だが、SMTP 設定・認証情報・宛先の未設定や不正により送信できない |

### 動作

- SMTP 設定や認証情報が未設定・不正でも llmlb は起動する。通知だけが無効になり、
  理由はログ（状態が変わったときに 1 回）と上記 API の `status.reason` に出る。
- TLS は必須。平文の SMTP と認証なしの SMTP には対応しない。
- メール本文に載せるエンドポイントの URL とエラー文からは、URL に埋め込まれた認証情報
  （`scheme://user:password@host` の `user:password@`）を除く。
- 送信時刻は壁時計基準で、再起動しても送信時刻はずれない。最終送信日を `settings` に
  永続化するため、同じ日に 2 回送ることはない。
- 送信時刻に停止していた場合は、起動後に当日分を 1 回だけ送る。次回は本来の送信時刻に戻る。
  当日の送信時刻を過ぎてから通知を有効にした場合も同じく、当日分をすぐに送る。
- 送信に失敗した場合は 15 分後に再試行する。失敗した日は送信済みとして記録しない。
  直近の失敗理由は `last_digest_error` で確認でき、次に成功すると `null` に戻る。
- 送信時刻をまたいで翌日の送信時刻より前まで停止していた場合、前日分は送らない
  （ダイジェストは送信時点の状態一覧であり、過去の日付の分は作れないため）。
- 設定の変更は再起動なしで 1 分以内に反映される。
- 即時通知は、エンドポイントが Offline 以外の状態から Offline に到達したときだけ送る。
  Error への遷移や復帰では送らない。同じエンドポイントについては、Offline 以外の状態を
  経由するまで再送しない（この状態はプロセス内で持つため、再起動直後は 1 通重複し得る）。
- 即時通知の送信に失敗した場合は再試行せず、ログに理由を出す。次の Offline 到達でもう一度送る。
- 単一インスタンスでの運用を前提とする（複数インスタンスでは同じメールが重複する）。
