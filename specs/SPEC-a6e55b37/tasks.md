# タスク: llmlb 自動アップデート（承認後に更新して再起動）

## Setup
- [x] T001 `specs/SPEC-a6e55b37/` のドキュメント整備（spec/plan/data-model/research/quickstart）
- [x] T002 Rust依存追加（`semver`, `tar`, `flate2`, `zip` 等）

## Tests (RED)
- [x] T010 inference gate: drain中 `/v1/*` が 503 になる integration test
- [x] T011 inference gate: in-flight が 0 になるまで待機できる unit test
- [x] T012 update manager: アセット選定（OS/arch→asset名）unit test

## Core
- [x] T020 `llmlb/src/inference_gate.rs` 追加（in-flight + reject + idle notify）
- [x] T021 `llmlb/src/shutdown.rs` 追加（Updateがgraceful shutdownを起動）
- [x] T022 `llmlb/src/update/` 追加（GitHub release取得、DL、状態管理、apply要求）
- [x] T023 `llmlb/src/cli` に内部コマンド `__internal` 追加（apply-update/run-installer）
- [x] T024 `llmlb/src/api/system.rs` 追加（GET /api/system, POST /api/system/update/apply）
- [x] T025 `llmlb/src/api/mod.rs` に system routes + `/v1` ミドルウェア適用
- [x] T026 `llmlb/src/main.rs` に UpdateManager の起動/統合（全OS）

## UI
- [x] T030 ダッシュボードに Update banner + Restart to update
- [x] T031 macOS/Windows トレイに Update 表示 + Restart to update
- [x] T032 `pnpm --filter @llm/dashboard build` で `llmlb/src/web/static/` を再生成

## Polish
- [x] T040 README 追記（自動アップデートの挙動/制約）
- [x] T041 ダッシュボードヘッダーに current version 常時表示を追加（Issue #415）
- [x] T042 `Dashboard.tsx`→`Header.tsx` へ `/api/system.version` の受け渡しを追加
- [x] T043 Playwright セレクタ/ページオブジェクトに `#current-version` を追加
- [x] T044 system-update/dashboard-header E2E を current version 検証に更新
