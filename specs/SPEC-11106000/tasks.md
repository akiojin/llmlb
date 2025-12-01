# タスク: SPEC-11106000 Hugging Face GGUFモデル対応登録

## 方針
- TDD順で進める。契約→Integration→E2E→Unitの順。
- Web/CLI/Routerの3面を並列化できるところは[P]マーク。

## Setup
- [ ] 環境変数で HF_TOKEN を設定できるようドキュメントを確認。

## Contract Tests (router)
- [ ] /api/models/available: HFモックでGGUF一覧を返す。検索・ページング・cachedフラグ検証。
- [ ] /api/models/register: 正常系（登録）と重複/404/URL欠損の異常系。
- [ ] /api/models/download: all/specific ターゲット、バリデーション。
- [ ] /api/tasks/{id}: ダウンロードタスクのステータス/進捗が返る。
- [ ] /v1/models: HF登録モデルが含まれる。

## Integration (router)
- [ ] HF API呼び出しのキャッシュ/429フォールバックをモックで確認。
- [ ] 登録→ダウンロードタスク生成→進捗更新の一連フロー。
- [ ] サイズ・GPU要件警告の付与（required_memory超過時）。

## Backend Implementation
- [x] ModelInfo/registry 拡張をDB永続化（HF登録モデルを保存・再起動復元）。
- [x] ModelInfo/registry 拡張（source/URL/last_modified/status）。
- [x] /api/models/available 実装（HF fetch + cache + pagination）。
- [x] /api/models/register 実装（ID命名: hf/repo/file）。
- [x] /api/models/download 実装（タスク生成、target all/specific）。
- [x] HF呼び出しに Bearer トークン対応（オプション）。
- [x] /v1/models に HF 登録分を統合。
- [ ] 構造化ログ・エラー整備。

## CLI
- [x] `llm-router model list` 実装（search/limit/offset/format）。
- [x] `llm-router model add <repo> --file <gguf>` 実装。
- [x] `llm-router model download <name> (--all | --node <uuid>)` 実装。
- [ ] CLIエラー/重複/進捗表示のテスト。

## Frontend (web/static)
- [x] 「対応可能モデル」「対応モデル」タブを分離表示。
- [x] HFカタログ一覧（検索/ソース表示/cached表示）。
- [x] 登録ボタンと状態表示（登録済み/重複抑止）。
- [x] 「今すぐダウンロード」（全ノード/指定ノード選択）UI。
- [x] ダウンロード進捗リスト（5秒ポーリング）。
- [ ] Download Tasks パネルの実データ反映確認と必要ならUI調整。

## Node (最小)
- [ ] manifest に HF 直URLが来ても downloadModel が扱えることを確認（必要ならURL判定を緩和）。

## E2E/Scenario
- [ ] カタログ→登録→全ノードダウンロード→/v1/modelsで利用 の一連を通すシナリオ。
- [ ] 429/障害時にキャッシュ結果が返るシナリオ。

## Docs
- [ ] README/CLAUDE.md に CLI/Web 手順を簡潔に追記。
- [ ] quickstart.md を最新UI/CLIに合わせて再確認。

## 検証
- [ ] cargo fmt/clippy/test、make quality-checks。
- [ ] markdownlint (specs含む)。
