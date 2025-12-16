# タスク: SPEC-11106000 Hugging Face GGUFモデル対応登録

## 方針
- TDD順で進める。契約→Integration→E2E→Unitの順。
- Web/CLI/Routerの3面を並列化できるところは[P]マーク。

## Setup
- [x] 環境変数で HF_TOKEN を設定できるようドキュメントを確認。

## Contract Tests (router)
- [x] /v0/models/register: 正常系（repo-only, file指定, GGUF/非GGUF）、重複/404。
- [x] 非GGUF→convertタスクが作成されること。
- [x] convert失敗→再キュー（Restore相当のAPI呼び出し）でタスクが新規作成され成功すること。
- [x] /v1/models: 実体があるものだけ返す（未ダウンロード・削除後は含まれない）。

## Integration (router)
- [x] HF siblingsモック→自動ファイル選択→convertキュー→（FAKEモードで）/v1/models に反映。
- [x] convert失敗時のエラー保持と再キュー成功の挙動を確認（APIベース）。
- [x] サイズ・GPU要件警告の付与（required_memory超過時）。

## Backend Implementation
- [x] ModelInfo/registry 拡張と永続化（repo/filename/source/status/path）。
- [x] /v0/models/register 実装（repo-only対応、GGUF優先、自動変換キュー、重複・404バリデーション）。
- [x] /v0/models/convert 実装（再キュー用エンドポイントを維持）。
- [x] convertマネージャ: 非GGUF→GGUF 変換（実行 or FAKE）、完了後にモデル登録を更新。
- [x] /v1/models は実体GGUFがあるものだけ返す。
- [x] 構造化ログ・エラー整備。

## CLI
- [x] `llm-router model list` 実装（search/limit/offset/format）。
- [x] `llm-router model add <repo> --file <gguf>` 実装。
- [x] `llm-router model download <name> (--all | --node <uuid>)` 実装。
- [x] CLIエラー/重複/進捗表示のテスト。

## Frontend (web/static)
- [x] HFカタログUIを削除/非表示にし、URL登録フォームのみ残す。
- [x] 登録済みモデル一覧（実体のみ）、削除ボタン。
- [x] Convertタスク一覧表示、失敗時に Restore ボタンで再キュー。
- [x] 登録・失敗バナーを × で閉じられ、4秒以上表示。
- [x] Restore ボタンのE2E/Playwrightテストを追加。

## Node (最小)
- [x] manifest に HF 直URL が来ても downloadModel が扱えることを確認。

## E2E/Scenario
- [x] URL登録（repo-only）→非GGUF→convert→/v1/models 反映→Restoreで再試行 の一連シナリオ（Playwrightでモック検証）。
- [x] 429/障害時にキャッシュ結果が返るシナリオ。

## Docs
- [x] quickstart.md をURL登録・Restore手順に更新。
- [x] tasks/plan/spec との整合確認（本タスクで更新）。

## 検証
- [x] cargo fmt/clippy/test、make quality-checks。
- [x] markdownlint (specs含む)。
