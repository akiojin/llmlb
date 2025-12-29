# タスク: SPEC-11106000 Hugging Face URL 登録（形式選択）

## 方針
- TDD順で進める。契約→Integration→E2E→Unitの順。
- Web/CLI/Routerの3面を並列化できるところは[P]マーク。

## Setup
- [x] 環境変数で HF_TOKEN を設定できるようドキュメントを確認。

## Contract Tests (router)
- [x] /v0/models/register: 正常系（repo-only, file指定, format必須/省略, gguf_policy）、重複/404。
- [x] `format=gguf` でGGUFが存在しない場合は 400 になること。
- [x] `format=safetensors` で `config.json`/`tokenizer.json` 不足時は 400 になること。
- [x] `format=safetensors` で `.safetensors` が複数かつ index 不在の場合は 400 になること。
- [x] /v1/models: 実体（safetensors/GGUF）があるものだけ返す（未ダウンロード・削除後は含まれない）。

## Integration (router)
- [x] HF siblingsモック→format選択→登録→/v1/models に反映。
- [x] gguf_policy が期待のGGUFを選択すること。
- [x] サイズ・GPU要件警告の付与（required_memory超過時）。

## Backend Implementation
- [x] ModelInfo/registry 拡張と永続化（format/gguf_policy/repo/filename/source/status/path）。
- [x] /v0/models/register 実装（format必須/省略判定、gguf_policy siblings選択、重複・404バリデーション）。
- [x] /v1/models は実体（safetensors/GGUF）があるものだけ返す。
- [x] 構造化ログ・エラー整備。

## CLI
- [x] `llm-router model list` 実装（search/limit/offset/format）。
- [x] `llm-router model add <repo> --file <gguf>` 実装。
- [x] `llm-router model download <name> (--all | --node <uuid>)` 実装。
- [x] CLIエラー/重複/進捗表示のテスト。

## Frontend (web/static)
- [x] HFカタログUIを削除/非表示にし、URL登録フォームのみ残す。
- [x] `format`/`gguf_policy` 選択UIと説明表示を追加。
- [x] 登録済みモデル一覧（実体のみ）、削除ボタン。
- [x] 登録・失敗バナーを × で閉じられ、4秒以上表示。
- [x] 形式選択/エラーのE2E/Playwrightテストを追加。

## Node (最小)
- [x] manifest に HF 直URL が来ても downloadModel が扱えることを確認。

## E2E/Scenario
- [x] URL登録（repo-only）→形式選択→/v1/models 反映の一連シナリオ（Playwrightでモック検証）。
- [x] GGUF無し/メタデータ不足時に明確なエラーが返ること。
- [x] 429/障害時にキャッシュ結果が返るシナリオ。

## Docs
- [x] quickstart.md をURL登録・形式選択の手順に更新。
- [x] tasks/plan/spec との整合確認（本タスクで更新）。

## 検証
- [x] cargo fmt/clippy/test、make quality-checks。
- [x] markdownlint (specs含む)。
