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
- [x] T041 ダッシュボードヘッダーに current version 常時表示を追加（Issue #415）
- [x] T042 `Dashboard.tsx`→`Header.tsx` へ `/api/system.version` の受け渡しを追加
- [x] T043 Playwright セレクタ/ページオブジェクトに `#current-version` を追加
- [x] T044 system-update/dashboard-header E2E を current version 検証に更新

## Polish
- [x] T040 README 追記（自動アップデートの挙動/制約）

## Regression Fixes (2026-02-19)
- [x] T050 `specs/SPEC-a6e55b37/spec.md` に手動更新チェック導線の回帰要件（User Story 5 / FR-006 / FR-007）を追記
- [x] T051 [P] `llmlb/tests/ui/update_banner.rs` に `Check for updates` 導線消失の回帰テストを追加（TDD）
- [x] T052 `llmlb/src/web/dashboard/src/pages/Dashboard.tsx` で `update` 未取得時でも Update banner を表示し、ボタン導線を維持

## Regression Fixes (2026-02-19 追補)
- [x] T053 `specs/SPEC-a6e55b37/spec.md` に手動チェック失敗時の `available` 状態保持要件（FR-009）と Restart表示条件要件（FR-010）を追記
- [x] T054 [P] `llmlb/src/update/mod.rs` に `record_check_failure` の回帰ユニットテストを追加（`available`保持 / 非`available`失敗遷移）
- [x] T055 [P] `llmlb/tests/ui/update_banner.rs` に Restart表示条件（`failed` かつ `latest` ありのみ）の回帰テストを追加
- [x] T056 `llmlb/src/update/mod.rs` と `llmlb/src/web/dashboard/src/pages/Dashboard.tsx` を修正し、payload保持とRestart導線ガードを実装

## UI Improvement: Update Queue Button States (2026-02-19)

### Setup
<!-- markdownlint-disable MD029 -->

- [x] T060 `specs/SPEC-a6e55b37/spec.md` にUS-6/US-7/US-8、FR-011〜FR-015を追記

### Tests (RED)

- [x] T061 [P] `llmlb/tests/ui/update_banner.rs` にdraining時ボタン表示テスト（テキスト/disabled/スピナー/in_flight反映）
- [x] T062 [P] `llmlb/tests/ui/update_banner.rs` にapplying時ボタン表示テスト（テキスト/disabled/スピナー）
- [x] T063 [P] `llmlb/tests/ui/update_banner.rs` にdraining/applying時のCheck for updatesのdisabledテスト
- [x] T064 [P] `llmlb/tests/ui/update_header.rs` にヘッダーアップデート状態バッジのテスト（各state→ドット色/バッジテキスト）

### Core

- [x] T070 `Dashboard.tsx` のRestart to updateボタンをdraining/applying状態に応じて動的テキスト・アイコン表示に変更
- [x] T071 `Dashboard.tsx` のCheck for updatesボタンのdisabled条件にdraining状態を追加
- [x] T072 `Header.tsx` にupdateState propsを追加し、ドットインジケータ+バッジを表示
- [x] T073 `Dashboard.tsx` からHeader.tsxにupdateState情報を渡すprops拡張

### Polish

- [x] T080 `pnpm --filter @llm/dashboard build` でstatic再生成、コミット対象に含める
- [x] T081 全品質チェック（`make quality-checks`）を実行・合格確認

## Force Update Flow (2026-02-20)

### Setup

- [x] T090 `specs/SPEC-a6e55b37/spec.md` にUS-9とFR-016〜FR-019を追記
- [x] T091 `specs/SPEC-a6e55b37/plan.md` に強制更新API/挙動を追記

### Tests (RED)

- [x] T092 `llmlb/src/update/mod.rs` に通常更新`queued`判定と強制更新ready条件のユニットテストを追加
- [x] T093 `llmlb/tests/integration/system_update_apply_api_test.rs` を追加し、`/api/system/update/apply` と `/api/system/update/apply/force` の契約を検証
- [x] T094 `llmlb/tests/ui/update_banner.rs` に強制更新ボタン表示・ready条件・`queued=false`分岐テストを追加

### Core

- [x] T095 `llmlb/src/update/mod.rs` で通常/強制更新要求モードを分離し、通常更新の`queued`判定と強制更新ready検証を実装
- [x] T096 `llmlb/src/update/mod.rs` の適用フローを更新し、通常更新で`in_flight=0`時に`draining`をスキップ、強制更新で`draining`を経由しないように実装
- [x] T097 `llmlb/src/api/system.rs` と `llmlb/src/api/mod.rs` に `POST /api/system/update/apply/force` を追加し、`mode`/`queued`レスポンスを返すように実装
- [x] T098 `llmlb/src/web/dashboard/src/lib/api.ts` と `llmlb/src/web/dashboard/src/pages/Dashboard.tsx` を更新し、`Force update now` ボタンと確認ダイアログを実装

## Regression Fixes (2026-02-22)

### Setup

- [x] T099 `specs/SPEC-a6e55b37/spec.md` のFR-018/US-9を更新し、`Force update now` を更新候補の有無に関係なく常時表示（条件未達時disabled）に明文化

### Tests (RED)

- [x] T100 `llmlb/tests/ui/update_banner.rs` に「更新未検知でも強制更新ボタンは表示される」回帰テストを追加

### Core

- [x] T101 `llmlb/src/web/dashboard/src/pages/Dashboard.tsx` を修正し、強制更新ボタンを常時表示 + `No update is available` 理由表示を追加

### Polish

- [x] T102 `pnpm --filter @llm/dashboard build` を実行して `llmlb/src/web/static/` を再生成し、埋め込み配信アセットへ反映
