# タスク: SPEC-99039000 APIキースコープ & /v0 認証強化

## 方針
- TDD順で進める（Contract → Integration → E2E → Unit）。
- 既存テストの更新もTDDの一部として扱う。

## Setup
- [x] 仕様の最新化（/v0認証必須、ユーザーロール追記）。

## Contract Tests (router)
- [x] APIキーのスコープ不足で403が返ることを検証。
- [x] `node:register` と `api:inference` の権限差を検証。
- [x] `/v0` 管理系APIは admin 以外を拒否することを検証。
- [x] `/v0/health` が `node:register` スコープとノードトークンを要求することを検証。

## Integration / E2E (router)
- [x] `/v0` 管理系/ダッシュボードAPIの認証必須化に合わせてテスト更新。
- [x] `/v1` 推論APIに `api:inference` スコープが必須であることを確認。

## Backend Implementation (router/common)
- [x] APIキーに `scopes` を追加しDBへ永続化。
- [x] APIキー認証/スコープ判定ミドルウェアを実装。
- [x] `/v0` 管理系ルートを admin（JWT or admin:*）に制限。
- [x] `/v0/nodes` を `node:register` スコープ必須に変更。
- [x] `/v0/models/blob/*` を `node:register` スコープ必須に変更。
- [x] デバッグ用 API キー（sk_debug*）のスコープ対応。
- [x] `/v0/health` を APIキー（`node:register`）必須に変更。

## Frontend (dashboard)
- [x] APIキー作成UIでスコープ選択を追加。
- [x] APIキー一覧にスコープ表示を追加。

## Node (C++)
- [x] `LLM_NODE_API_KEY` を設定可能にする。
- [x] ノード登録時に APIキーを送信。
- [x] モデル配信 (`/v0/models/blob`) に APIキーを送信。
- [x] ハートビート (`/v0/health`) に APIキーを送信。

## Docs
- [x] README / README.ja に権限マトリクスと環境変数を追記。
- [x] `docs/authentication.md` を更新（スコープ/デバッグキー）。
- [x] `/v0/health` のAPIキー必須化に合わせてドキュメントを更新。

## 検証
- [x] `cargo fmt --check`
- [x] `cargo clippy -- -D warnings`
- [x] `cargo test`
- [x] `.specify/scripts/checks/check-tasks.sh`
- [x] `pnpm dlx markdownlint-cli2 "**/*.md" "!node_modules" "!.git" "!.github" "!.worktrees"`
