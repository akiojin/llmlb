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

## Phase 2: 応答性・スケジュール・ロールバック (2026-02-23)

### Setup

- [x] T200 `specs/SPEC-a6e55b37/spec.md` に Phase 2 要件（US-10〜US-19, FR-020〜FR-042, NFR-004〜NFR-007）を追記
- [x] T201 `specs/SPEC-a6e55b37/plan.md` に Phase 2 実装計画を追記
- [x] T202 `specs/SPEC-a6e55b37/tasks.md` に Phase 2 タスクを追加

### Phase 2a: 応答性改善 — Tests (RED)
<!-- markdownlint-disable MD024 -->

- [x] T210 [P] `check_only` のユニットテスト（GitHub APIチェックのみ同期、5秒以内にレスポンス）
- [x] T211 [P] `download_background` のユニットテスト（バックグラウンドDL開始、PayloadState進捗更新）
- [x] T212 [P] レートリミット判定のユニットテスト（60秒以内連打→拒否、60秒経過→許可）
- [x] T213 `POST /api/system/update/check` のインテグレーションテスト（チェックのみ同期応答、DLバックグラウンド開始、429レートリミット）

### Phase 2a: 応答性改善 — Core

- [x] T220 `check_and_maybe_download` を `check_only`（同期、5秒以内）と `download_background`（非同期）に分離
- [x] T221 `PayloadState::Downloading` に `downloaded_bytes` / `total_bytes` フィールドを追加
- [x] T222 `download_to_path` にストリーミングDL＋進捗コールバックを実装
- [x] T223 `POST /api/system/update/check` をチェックのみに変更（DLはバックグラウンド自動開始）
- [x] T224 サーバー側レートリミット実装（手動チェック最小60秒間隔、超過時429）

### Phase 2b: スケジューリング — Tests (RED)

- [x] T230 [P] `UpdateSchedule` 構造体と `update-schedule.json` のシリアライズ/デシリアライズのユニットテスト
- [x] T231 [P] `update-history.json` の書き込み（追加・上限100件維持）のユニットテスト
- [x] T232 [P] アイドル時適用トリガー（in_flight=0で即座に適用開始）のユニットテスト
- [x] T233 [P] 時刻指定適用トリガー（指定時刻到達でドレイン開始）のユニットテスト
- [x] T234 `POST /api/system/update/schedule` のインテグレーションテスト（予約作成、コンフリクト409、キャンセル）
- [x] T235 `GET /api/system/update/schedule` と `DELETE /api/system/update/schedule` のインテグレーションテスト

### Phase 2b: スケジューリング — Core

- [x] T240 `UpdateSchedule` 構造体と `update-schedule.json` の読み書きを実装
- [x] T241 `UpdateHistory` 構造体と `update-history.json` の読み書きを実装（直近100件、追記）
- [x] T242 `POST /api/system/update/schedule` API実装（mode/datetime指定、コンフリクト検知409）
- [x] T243 `GET /api/system/update/schedule` API実装（現在の予約状態取得）
- [x] T244 `DELETE /api/system/update/schedule` API実装（予約キャンセル、予約なし時404）
- [x] T245 アイドル時適用ロジック実装（`InferenceGate::wait_for_idle` 監視、in_flight=0でトリガー）
- [x] T246 時刻指定適用ロジック実装（`tokio::time::sleep_until` で指定時刻にドレイン→適用）
- [x] T247 `GET /api/system` レスポンスに `schedule` フィールドを追加
- [x] T248 予約の再起動後復元ロジック実装（起動時に `update-schedule.json` を読み込み再開）

### Phase 2c: ドレインタイムアウト — Tests (RED)

- [x] T250 [P] ドレインタイムアウト（300秒超過でキャンセル＋ゲート再開＋failed遷移）のユニットテスト
- [x] T251 ドレインタイムアウトのインテグレーションテスト（タイムアウト超過で503ゲート解除確認）

### Phase 2c: ドレインタイムアウト — Core

- [x] T255 `UpdateState::Draining` に `timeout_at` を追加し、ドレインにタイムアウト（デフォルト300秒）を実装
- [x] T256 タイムアウト超過時のドレインキャンセル＋ゲート再開＋`failed`遷移を実装

### Phase 2d: ロールバック — Tests (RED)

- [x] T260 [P] ヘルパープロセスの起動監視（30秒以内にヘルスチェック無応答→`.bak`から復元）のユニットテスト
- [x] T261 [P] `POST /api/system/update/rollback` のインテグレーションテスト（`.bak`存在時受理、なし時409）
- [x] T262 [P] ロールバック結果の `update-history.json` 記録テスト

### Phase 2d: ロールバック — Core

- [x] T265 ヘルパープロセス（`__internal apply-update`）に起動監視を追加（30秒ヘルスチェック→`.bak`復元）
- [x] T266 `POST /api/system/update/rollback` API実装（`.bak`存在時のみ受理）
- [x] T267 ロールバック結果を `update-history.json` に記録する処理を追加
- [x] T268 `GET /api/system` レスポンスに `rollback_available` フィールドを追加

### Phase 2e: バグ修正 — Tests (RED)

- [x] T270 `GET /api/system` がリリースビルドで正常応答を返すことのインテグレーションテスト

### Phase 2e: バグ修正 — Core

- [x] T275 `GET /api/system` が一部リリースビルド環境で401を返す問題を調査・修正（`Current v--` 表示の原因）

### Phase 2f: Dashboard UI — Tests (RED)

- [x] T280 [P] DL進捗プログレスバー表示テスト（PayloadState::Downloadingの進捗率表示）
- [x] T281 [P] viewerロール判定テスト（Update banner・操作ボタン非表示、ヘッダーバージョンのみ表示）
- [x] T282 [P] 手動チェックUIスロットリング（30秒以内の連打でボタンdisabled）テスト
- [x] T283 [P] ドレインタイムアウトカウントダウン表示テスト
- [x] T284 [P] 手動ロールバックボタン表示条件（`.bak`存在時のみ有効＋確認ダイアログ）テスト
- [x] T285 [P] アップデート設定モーダルテスト（タブ切替・モード選択・日時ピッカー・履歴表示）

### Phase 2f: Dashboard UI — Core

- [x] T290 `system.ts` の型定義に Phase 2 フィールド（DL進捗、schedule、rollback_available）を追加
- [x] T291 DL進捗プログレスバーコンポーネントの実装（バイト数＋パーセンテージ表示）
- [x] T292 viewerロール判定実装（Update banner・操作ボタン非表示、ヘッダーバージョンのみ表示）
- [x] T293 手動チェックのUIスロットリング実装（最小30秒間隔、タイムスタンプ管理）
- [x] T294 ドレインタイムアウトカウントダウン表示の実装
- [x] T295 手動ロールバックボタン（`.bak`存在時のみ有効）＋確認ダイアログの実装
- [x] T296 アップデート設定モーダルの実装（適用モード選択・日時ピッカー・予約状態・履歴タブ）
- [x] T297 Update bannerに予約状態（予約者名・モード・予約時刻）を表示
- [x] T298 スケジュールAPI（create/cancel/get）のクライアント実装（`system.ts` に追加）

### Phase 2g: Tray

- [ ] T300 macOS/Windowsトレイに予約状態通知を追加（「明日AM3:00に更新予定」等）

### Phase 2: Polish

- [ ] T310 `pnpm --filter @llm/dashboard build` でstatic再生成、コミット対象に含める
- [ ] T311 全品質チェック（`make quality-checks`）を実行・合格確認
- [ ] T312 README追記（Phase 2機能: スケジュール・ロールバック・進捗表示）
