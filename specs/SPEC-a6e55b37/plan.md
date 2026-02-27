# 実装計画: llmlb 自動アップデート（承認後に更新して再起動）

## Summary

llmlb が GitHub Releases の更新を検知し、ユーザーが承認した場合に「新規推論リクエストを停止→処理中完了を待つ→更新適用→再起動」を行う。加えて緊急時向けに、処理中リクエストの完走を待たない強制更新フローを提供する。更新の適用方法は OS とインストール形態により分岐し、Ollama と同様に "可能な範囲で自動、無理なら手動誘導" とする。

**Phase 2** では、チェックの応答性改善、アップデートスケジューリング（即時/アイドル時/時刻指定）、予約永続化と履歴、自動・手動ロールバック、ドレインタイムアウト、DL進捗表示、viewerロール対応、レートリミット保護を追加する。

## 方針

- 推論 `/v1/*` の **in-flight** を正確に数える（クラウドモデル含む）。
- 承認後は `/v1/*` を 503 で拒否し、in-flight が 0 になるまで待機（ドレイン）。
- 強制更新時は `/v1/*` を 503 で拒否し、in-flight を待たずに適用へ進む。
- 更新適用は内部アップデータ（別プロセス）で行い、**実行中ファイル置換不可**を回避する。
- macOS/Windows はインストーラ方式の更新（pkg/msi）を優先する経路も持つ（権限が必要ならプロンプト）。
- Linux は書き込み可能な配置のときのみポータブル置換で更新し、それ以外は手動誘導。
- **Phase 2**: チェックはGitHub APIのみ同期、DLはバックグラウンド。進捗はポーリングで取得。
- **Phase 2**: 予約・履歴はJSONファイルベース（SQLite非依存）。

## Phase 1 変更点（実装済み）

### Rust (llmlb)

- `llmlb/src/update/` の追加（UpdateManager、GitHub release取得、アセット選定、DL/展開、適用要求）
- `llmlb/src/inference_gate.rs` の追加（in-flight カウント + 503ゲート + idle待機）
- `llmlb/src/shutdown.rs` の追加（UpdateManager が graceful shutdown を起動できる仕組み）
- `llmlb/src/api/system.rs` の追加
- `llmlb/src/api/mod.rs` への system routes と `/v1` ミドルウェアの組み込み
- `llmlb/src/cli/` へ内部コマンド `__internal` の追加（apply-update/run-installer）
- `llmlb/src/gui/tray.rs` 更新（Update表示 + Restart to update）

### Dashboard

- `GET /api/system` をポーリングして Update banner を表示
- `Restart to update`（通常更新）と `Force update now`（強制更新）と `Open Releases` の導線
- 変更後に `pnpm --filter @llm/dashboard build` で `llmlb/src/web/static/` を再生成

## Phase 2 変更点（計画）

### Rust (llmlb) — 応答性改善

- `check_and_maybe_download` を分離: `check_only` (同期、5秒以内) + `download_background` (非同期)
- `PayloadState::Downloading` に `downloaded_bytes`/`total_bytes` を追加
- `download_to_path` にストリーミングDL＋進捗コールバックを追加
- `POST /api/system/update/check` をチェックのみに限定（DLはバックグラウンド自動開始）
- サーバー側レートリミット: 手動チェック最小60秒間隔、超過時429

### Rust (llmlb) — スケジューリング

- `UpdateSchedule` 構造体: mode（immediate/idle/scheduled）、scheduled_at、scheduled_by、target_version
- `update-schedule.json` の読み書き（予約永続化）
- `update-history.json` の読み書き（履歴永続化、直近100件）
- `POST /api/system/update/schedule` — 予約作成（mode/datetime指定）
- `DELETE /api/system/update/schedule` — 予約キャンセル
- `GET /api/system/update/schedule` — 現在の予約状態取得
- アイドル時適用: `InferenceGate::wait_for_idle` を監視し、in_flight=0でトリガー
- 時刻指定適用: `tokio::time::sleep_until` で指定時刻にドレイン→適用

### Rust (llmlb) — ドレインタイムアウト

- ドレインにタイムアウト（デフォルト300秒）を追加
- タイムアウト超過時はドレインキャンセル＋ゲート再開＋`failed`遷移
- `UpdateState::Draining` に `timeout_at` を追加

### Rust (llmlb) — ロールバック

- ヘルパープロセス（`__internal apply-update`）に起動監視を追加
  - 新プロセスspawn後、30秒以内に `GET /api/version` でヘルスチェック
  - 応答なし/バージョン不一致の場合、`.bak` から復元して再起動
- `POST /api/system/update/rollback` — 手動ロールバックAPI（`.bak`存在時のみ）
- ロールバック結果を `update-history.json` に記録

### Rust (llmlb) — バグ修正

- `GET /api/system` が一部リリースビルド環境で401を返す問題を調査・修正
  （`Current v--` 表示の原因）

### Dashboard — Phase 2 UI

- `POST /api/system/update/check` の応答に合わせてスピナー表示を短縮
- DL進捗プログレスバー（バイト数＋パーセンテージ）の追加
- アップデート設定モーダル（適用モード選択・日時ピッカー・予約状態・履歴タブ）
- Update bannerに予約状態（予約者名・モード・予約時刻）を表示
- viewerロール判定: Update banner・操作ボタンを非表示にし、ヘッダーバージョンのみ表示
- 手動チェックのUIスロットリング（最小30秒間隔）
- ドレインタイムアウトのカウントダウン表示
- 手動ロールバックボタン（`.bak`存在時のみ有効）＋確認ダイアログ

### Tray — Phase 2

- 予約状態通知（「明日AM3:00に更新予定」等）

## API / I/O

### `GET /api/system`

- `version`: 現行バージョン
- `pid`: サーバーPID
- `in_flight`: 推論 in-flight 数（ゲートで計測）
- `update`: `state` tagged union（snake_case）
  - `up_to_date`: `checked_at?`
  - `available`: `current`, `latest`, `release_url`, `portable_asset_url?`, `installer_asset_url?`, `payload`, `checked_at`
    - `payload` は `payload` tagged union（snake_case）
      - `not_ready`
      - `downloading`: `started_at`, `downloaded_bytes?`, `total_bytes?`
      - `ready`: `kind`（`portable`/`installer`）
      - `error`: `message`
  - `draining`: `latest`, `in_flight`, `requested_at`, `timeout_at`
  - `applying`: `latest`, `method`（`portable_replace|mac_pkg|windows_setup`）
  - `failed`: `latest?`, `release_url?`, `message`, `failed_at`
- `schedule`: スケジュール情報（予約がある場合）
  - `mode`: `immediate|idle|scheduled`
  - `scheduled_at?`: ISO8601（時刻指定の場合）
  - `scheduled_by`: 予約者ユーザー名
  - `target_version`: 対象バージョン
  - `created_at`: 予約作成日時
- `rollback_available`: `.bak` が存在するかどうか

### `POST /api/system/update/check`

- Phase 2: GitHubチェックのみ同期実行（5秒以内）。DLはバックグラウンド。
- レスポンス: `{ update: UpdateState }`
- レートリミット超過時: `429 Too Many Requests`

### `POST /api/system/update/apply`

- 通常更新要求。`queued` を返し、`false` の場合は `draining` を経由せず直接 `applying` へ遷移（`202 Accepted`）
- 既に予約が存在する場合: `409 Conflict`（予約情報を含む）

### `POST /api/system/update/apply/force`

- 強制更新要求。`available` かつ `payload=ready` の場合のみ受理し、`draining` を経由せず即時適用へ遷移（`202 Accepted`）
- 条件を満たさない場合は `409 Conflict`

### `POST /api/system/update/schedule` (Phase 2)

- Body: `{ mode: "idle"|"scheduled", scheduled_at?: string }`
- 予約作成。既存予約がある場合は `409 Conflict`
- レスポンス: `{ schedule: ScheduleInfo }`

### `DELETE /api/system/update/schedule` (Phase 2)

- 予約キャンセル。予約がない場合は `404 Not Found`
- レスポンス: `{ cancelled: true }`

### `GET /api/system/update/schedule` (Phase 2)

- 現在の予約状態を返す。予約がない場合は `{ schedule: null }`

### `POST /api/system/update/rollback` (Phase 2)

- `.bak` から前バージョンに戻す。`.bak` がない場合は `409 Conflict`
- レスポンス: `{ rolling_back: true }`

## 例外/失敗時

- ダウンロード失敗: `failed` 状態 + `Open Releases` 導線。サービスは継続。
- 権限不足: `failed` 状態 + 手動手順（macOS pkg / Windows msi / Linux tar.gz 配置）。
- ドレインタイムアウト: `failed` 状態 + ゲート再開。サービスは継続。
- 起動失敗: 自動ロールバック（ヘルパーが30秒監視）。前バージョンで再起動。
- 予約コンフリクト: `409 Conflict` + 既存予約情報を返す。

## テスト計画

### Phase 1（実装済み）

- Unit: アセット選定、バージョン比較、状態遷移
- Integration: `/v1/*` 503 ゲート、in-flight のドレイン待機、`POST apply` の `queued` 応答、`POST apply/force` の受理/拒否
- 可能な範囲で内部アップデータの "引数バリデーション/待機ロジック" をテスト（実インストーラの実行はテスト外）

### Phase 2（計画）

- Unit: スケジュール永続化（JSON読み書き）、履歴記録、ドレインタイムアウト、レートリミット判定
- Unit: チェック/ダウンロード分離（チェックのみ→DLバックグラウンド開始）
- Integration: `POST /api/system/update/schedule` の予約作成/コンフリクト/キャンセル
- Integration: `POST /api/system/update/rollback` の受理/拒否
- Integration: ドレインタイムアウト（300秒超過でキャンセル）
- Integration: レートリミット（60秒以内連打で429）
- UI: viewerロール判定（Update banner非表示）
- UI: DL進捗プログレスバー表示
- UI: アップデート設定モーダル（モード選択・日時ピッカー・履歴タブ）
- UI: ドレインタイムアウトカウントダウン表示
- UI: ロールバックボタン表示条件
