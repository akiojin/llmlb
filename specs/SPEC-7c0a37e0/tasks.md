# タスク: SPEC-k5mdhprl APIキースコープ & /api 認証強化

**ステータス**: 完了

## 方針
- TDD順で進める（Contract → Integration → E2E → Unit）。
- 既存テストの更新もTDDの一部として扱う。

## Setup
- [x] 仕様の最新化（/api認証必須、ユーザーロール追記）。

## 追加対応（Session 2025-12-31）

- [x] マニフェスト取得（`/api/models/registry/:model_name/manifest.json`）を`node`スコープ必須に更新
- [x] Node がマニフェスト取得時に APIキーを送信

## Contract Tests (router)
- [x] [P] APIキーのスコープ不足で403が返ることを検証。
- [x] [P] `node` と `api` の権限差を検証。
- [x] [P] `/api` 管理系APIは admin 以外を拒否することを検証。
- [x] [P] `/api/health` が `node` スコープとノードトークンを要求することを検証。

## Integration / E2E (router)
- [x] [P] `/api` 管理系/ダッシュボードAPIの認証必須化に合わせてテスト更新。
- [x] [P] `/v1` 推論APIに `api` スコープが必須であることを確認。

## Backend Implementation (llmlb/common)
- [x] APIキーに `scopes` を追加しDBへ永続化。
- [x] APIキー認証/スコープ判定ミドルウェアを実装。
- [x] `/api` 管理系ルートを admin（JWT or admin）に制限。
- [x] `/api/nodes` を `node` スコープ必須に変更。
- [x] `/api/models/blob/*` を `node` スコープ必須に変更（旧仕様）。
- [x] デバッグ用 API キー（sk_debug*）のスコープ対応。
- [x] `/api/health` を APIキー（`node`）必須に変更。

## Frontend (dashboard)
- [x] [P] APIキー作成UIでスコープ選択を追加。
- [x] [P] APIキー一覧にスコープ表示を追加。

## Node (C++)
- [x] `XLLM_API_KEY` を設定可能にする。
- [x] ノード登録時に APIキーを送信。
- [x] モデル配信 (`/api/models/blob`) に APIキーを送信（旧仕様）。
- [x] ハートビート (`/api/health`) に APIキーを送信。

## Docs
- [x] [P] README / README.ja に権限マトリクスと環境変数を追記。
- [x] [P] `docs/authentication.md` を更新（スコープ/デバッグキー）。
- [x] [P] `/api/health` のAPIキー必須化に合わせてドキュメントを更新。

## 検証
- [x] [P] `cargo fmt --check`
- [x] [P] `cargo clippy -- -D warnings`
- [x] [P] `cargo test`
- [x] [P] `.specify/scripts/checks/check-tasks.sh`
- [x] [P] `pnpm dlx markdownlint-cli2 "**/*.md" "!node_modules" "!.git" "!.github" "!.worktrees"`
