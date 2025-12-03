# USAGE: OpenAI互換エンドポイント

## 基本
- `POST /v1/chat/completions`
- `POST /v1/completions`
- `POST /v1/embeddings`

## クラウドモデルプレフィックス
- 付けるだけでクラウド経路に切替: `openai:`, `google:`, `anthropic:`（`ahtnorpic:` も許容）
- 例: `model: "openai:gpt-4o"` / `model: "google:gemini-1.5-pro"` / `model: "anthropic:claude-3-opus"`
- 転送時にプレフィックスは除去され、クラウドAPIへそのまま送られます。
- プレフィックスなしのモデルは従来どおりローカルLLMにルーティングされます。

## 環境変数
- `OPENAI_API_KEY`（必須）、`OPENAI_BASE_URL`（任意, default `https://api.openai.com`）
- `GOOGLE_API_KEY`（必須）、`GOOGLE_API_BASE_URL`（任意, default `https://generativelanguage.googleapis.com/v1beta`）
- `ANTHROPIC_API_KEY`（必須）、`ANTHROPIC_API_BASE_URL`（任意, default `https://api.anthropic.com`）

## ストリーミング
- `stream: true` でクラウドSSE/チャンクをそのままパススルー。

## メトリクス
- `GET /metrics/cloud` （Prometheus text）
  - `cloud_requests_total{provider,status}`
  - `cloud_request_latency_seconds{provider}`

## エラー方針（抜粋）
- APIキー未設定: 400（`*_API_KEY is required ...`）
- 不明プレフィックス: 400
- クラウド側4xx/5xx: ステータスとボディをそのまま返却（ヘッダも維持）
