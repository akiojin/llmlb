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
