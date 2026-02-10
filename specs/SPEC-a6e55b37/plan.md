# 実装計画: llmlb 自動アップデート（承認後に更新して再起動）

## Summary
llmlb が GitHub Releases の更新を検知し、ユーザーが承認した場合に「新規推論リクエストを停止→処理中完了を待つ→更新適用→再起動」を行う。更新の適用方法は OS とインストール形態により分岐し、Ollama と同様に “可能な範囲で自動、無理なら手動誘導” とする。

## 方針
- 推論 `/v1/*` の **in-flight** を正確に数える（クラウドモデル含む）。
- 承認後は `/v1/*` を 503 で拒否し、in-flight が 0 になるまで待機（ドレイン）。
- 更新適用は内部アップデータ（別プロセス）で行い、**実行中ファイル置換不可**を回避する。
- macOS/Windows はインストーラ方式の更新（pkg/msi）を優先する経路も持つ（権限が必要ならプロンプト）。
- Linux は書き込み可能な配置のときのみポータブル置換で更新し、それ以外は手動誘導。

## 主な変更点
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
- `Restart to update`（POST）と `Open Releases` の導線
- 変更後に `pnpm --filter @llm/dashboard build` で `llmlb/src/web/static/` を再生成

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
      - `downloading`: `started_at`
      - `ready`: `kind`（`portable`/`installer`）
      - `error`: `message`
  - `draining`: `latest`, `in_flight`, `requested_at`
  - `applying`: `latest`, `method`（`portable_replace|mac_pkg|windows_msi`）
  - `failed`: `latest?`, `release_url?`, `message`, `failed_at`

### `POST /api/system/update/apply`
- 非同期で `draining` 開始 → 適用 → 再起動（`202 Accepted`）

## 例外/失敗時
- ダウンロード失敗: `failed` 状態 + `Open Releases` 導線。サービスは継続。
- 権限不足: `failed` 状態 + 手動手順（macOS pkg / Windows msi / Linux tar.gz 配置）。
- ドレインが終わらない: 状態として観測できる（運用者が判断して停止可能）。

## テスト計画
- Unit: アセット選定、バージョン比較、状態遷移
- Integration: `/v1/*` 503 ゲート、in-flight のドレイン待機、`POST apply` がドレインに入る
- 可能な範囲で内部アップデータの “引数バリデーション/待機ロジック” をテスト（実インストーラの実行はテスト外）
