# 実装計画: MCPサーバーのCLI完全移行

**機能ID**: `SPEC-08af4e3c` | **日付**: 2026-02-17 | **仕様**: `specs/SPEC-08af4e3c/spec.md`
**入力**: `/specs/SPEC-08af4e3c/spec.md` の機能仕様

## 概要
TypeScript製MCPサーバーを廃止し、同等機能をRust製 `llmlb` CLIサブコマンド `assistant` として統合する。あわせて、Claude CodeプラグインとCodexスキルを新設し、npm配布導線を削除する。

## 技術コンテキスト
**言語/バージョン**: Rust (workspace設定), Markdown/JSON
**主要依存関係**: clap, reqwest, serde_json, regex, anyhow
**ストレージ**: なし（CLI処理中心）
**テスト**: cargo test（CLI parsing + assistantロジックのunit test）
**対象プラットフォーム**: Windows/macOS/Linux
**プロジェクトタイプ**: single（既存 `llmlb` へ統合）
**制約**: npm公開廃止、既存MCPの安全制約を維持、README/CI整合
**スコープ**: CLI機能移植、配布導線変更、plugin/skill追加

## 実装方針
1. `llmlb/src/cli/assistant.rs` を新設し、`curl/openapi/guide` サブコマンドを実装する。
2. `mcp-server/src/tools/execute-curl.ts` の挙動（sanitizer/host-validator/auth injection/result format）をRustへ移植する。
3. `main.rs` で `Commands::Assistant` を処理する。
4. `mcp-server/` を削除し、workspace/CI/release/scripts/READMEから関連参照を除去する。
5. Claudeプラグイン (`.claude-plugin`) とローカルミラー (`.claude/skills`) を追加する。
6. Codexスキル (`.codex/skills`) を追加し、`codex-skills/dist` を出力先としてドキュメント化する。

## 変更対象
- `llmlb/src/cli/mod.rs`
- `llmlb/src/cli/assistant.rs` (新規)
- `llmlb/src/main.rs`
- `llmlb/tests/cli_tests.rs`
- `package.json`
- `pnpm-workspace.yaml`
- `.mcp.json`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `scripts/publish.sh`
- `README.md`, `README.ja.md`
- `.claude-plugin/**` (新規)
- `.claude/skills/llmlb-cli-usage/SKILL.md` (新規)
- `.codex/skills/llmlb-cli-usage/SKILL.md` (新規)
- `codex-skills/dist/` (ディレクトリ新設)
- `mcp-server/` (削除)

## テスト戦略
1. 既存CLIパーステストに `assistant` 系を追加。
2. `assistant.rs` 内で sanitizer/validator/auth injection/OpenAPI/guide のunit testを追加。
3. `cargo test -p llmlb --test cli_tests` および `cargo test -p llmlb assistant` を実行。
4. `rg` で `@llmlb/mcp-server`, `llmlb-mcp`, `npm publish` の残存チェック。

## 受け入れ基準
- `llmlb assistant` が旧MCPの主要機能をCLIで提供している。
- `mcp-server/` が存在しない。
- npm公開ジョブがCI/Releaseから削除されている。
- Claude plugin/Codex skillの定義が追加されている。
- READMEが新運用手順へ更新されている。
