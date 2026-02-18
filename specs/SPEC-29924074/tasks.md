# タスク: MCPサーバーのCLI完全移行

**入力**: `specs/SPEC-29924074/` の設計ドキュメント
**前提条件**: spec.md, plan.md, research.md, data-model.md

## Phase 1: Foundation
- [x] T001 `specs/SPEC-29924074/` の仕様・計画・設計ファイルを作成
- [x] T002 `llmlb/src/cli/mod.rs` に `assistant` サブコマンドを追加
- [x] T003 `llmlb/src/main.rs` に `Commands::Assistant` 実行経路を追加

## Phase 2: Assistant CLI implementation
- [x] T004 `llmlb/src/cli/assistant.rs` を新規作成し `curl/openapi/guide` を実装
- [x] T005 sanitizer/validator/auth header injection ロジックを実装
- [x] T006 command masking / timeout / JSON整形を実装
- [x] T007 `llmlb/tests/cli_tests.rs` に assistant parse テストを追加
- [x] T008 `assistant.rs` に unit test を追加

## Phase 3: Remove MCP and npm publish path
- [x] T009 `mcp-server/` ディレクトリを削除
- [x] T010 `package.json` / `pnpm-workspace.yaml` / `.mcp.json` のmcp参照を削除
- [x] T011 `.github/workflows/ci.yml` から mcp-server job を削除
- [x] T012 `.github/workflows/release.yml` から npm publish job を削除
- [x] T013 `scripts/publish.sh` のmcp/npm関連処理を削除

## Phase 4: Claude plugin + Codex skill
- [x] T014 `.claude-plugin/marketplace.json` を新規追加
- [x] T015 `.claude-plugin/plugins/llmlb-cli/plugin.json` を新規追加
- [x] T016 `.claude-plugin/plugins/llmlb-cli/skills/llmlb-cli-usage/SKILL.md` を新規追加
- [x] T017 `.claude/skills/llmlb-cli-usage/SKILL.md` を追加（ミラー）
- [x] T018 `.codex/skills/llmlb-cli-usage/SKILL.md` を追加
- [x] T019 `codex-skills/dist/` を追加しパッケージ出力先を確保

## Phase 5: Documentation and verification
- [x] T020 `README.md` をCLI移行内容に更新
- [x] T021 `README.ja.md` をCLI移行内容に更新
- [x] T022 `README.md` のプロジェクト構成から `mcp-server/` を除去
- [x] T023 `cargo test` と参照検索で受け入れ条件を検証
