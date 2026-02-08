# タスク: SPEC-7c0a37e0 APIキー権限（Permissions）& /api 認証強化

**ステータス**: ✅ 完了（`scopes` 廃止 → `permissions` へ移行）

## 方針

- TDD順で進める（Contract → Integration → E2E → Unit）。
- 仕様（`spec.md` / `data-model.md` / `contracts/openapi.yaml`）と実装・テストの整合を最優先する。

## Backend / DB

- [x] DB: `api_keys.permissions` を追加し、旧`scopes`から`permissions`へ移行（マイグレーション追加）。
- [x] Common: `ApiKeyPermission` と `ApiKey.permissions` を追加。
- [x] DB層: `permissions` の保存/読み取りを実装（NULL/不正値は default-deny）。
- [x] Middleware: `permissions` ベースの認可へ移行（JWT or API key を統一）。
- [x] Router: `/api/dashboard/*` を JWT のみに制限（APIキー不可）。
- [x] API: `/api/api-keys` は `permissions` のみ受け付け、`scopes` は 400 で拒否。

## Frontend (dashboard)

- [x] APIキー作成UIを permissions（チェックボックス）へ移行。
- [x] Vite build を実行し、埋め込み静的アセット（`llmlb/src/web/static`）を更新。

## Specs / Docs

- [x] SPEC-7c0a37e0 を permissions 仕様へ更新（spec/data-model）。
- [x] SPEC-7c0a37e0 の OpenAPI contract を permissions へ更新。
- [x] SPEC-d4eb8796 の quickstart/tasks を permissions へ更新。
- [x] `docs/authentication.md` を permissions へ更新。
- [x] `README.md` / `README.ja.md` の API仕様・認証マトリクスを現行（endpoints/permissions）に更新。

## Tests / Verification

- [x] Rust tests: `permissions` 仕様に合わせて Contract/Integration/E2E を更新。
- [x] `make quality-checks` を実行し、全てグリーンであることを確認。
- [x] Playwright: E2E walkthrough（`llmlb/tests/e2e-playwright`）を実行し、主要画面が通ることを確認。
