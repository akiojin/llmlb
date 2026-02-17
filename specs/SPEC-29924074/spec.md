# 機能仕様書: MCPサーバーのCLI完全移行

**機能ID**: `SPEC-29924074`
**作成日**: 2026-02-17
**ステータス**: 完了
**入力**: ユーザー説明: "MCPサーバーをCLIへ完全移行し、Claude Code向けプラグインとCodex向けスキルとして実装。npm配布は廃止。"

## ユーザーシナリオ＆テスト

### 主要ユーザーストーリー
運用者は、既存のMCPサーバー経由の操作を廃止し、`llmlb` 標準CLIのみで同等の補助操作を実行したい。AIアシスタント利用者は、Claude CodeプラグインおよびCodexスキル経由で新しいCLI手順を利用したい。配布担当者はnpm publishを停止し、誤って再配布されない状態にしたい。

### 受け入れシナリオ
1. **前提** llmlbバイナリが利用可能、**実行** `llmlb assistant curl --command "curl http://localhost:32768/v1/models"` を実行、**結果** 旧 `execute_curl` 相当の安全制約・認証補完付きでJSON結果を得られる。
2. **前提** `docs/openapi.yaml` が存在、**実行** `llmlb assistant openapi` を実行、**結果** OpenAPI JSONが標準出力に表示される。
3. **前提** リポジトリを参照、**実行** ClaudeプラグインとCodexスキル定義を確認、**結果** CLI利用手順が両方に定義されている。
4. **前提** リポジトリCI設定を参照、**実行** `@llmlb/mcp-server` と `npm publish` を検索、**結果** npm配布導線が存在しない。

### エッジケース
- 不正なcurlコマンド（シェル注入、危険オプション）を渡した場合は安全に拒否されること。
- 許可されていないホストへのアクセスは拒否されること。
- `LLMLB_API_KEY` / `LLMLB_ADMIN_API_KEY` が未設定でもコマンドがクラッシュせず、注入可能時のみ認証ヘッダーが自動付与されること。
- OpenAPIファイルが読めない場合でも組み込み仕様へフォールバックすること。

## 要件

### 機能要件
- **FR-001**: システムはMCPサーバーの `execute_curl` 機能を `llmlb assistant curl` で提供する必要がある。
- **FR-002**: システムはMCPリソースのOpenAPI提供機能を `llmlb assistant openapi` で提供する必要がある。
- **FR-003**: システムはMCPリソースのAPIガイド提供機能を `llmlb assistant guide` で提供する必要がある。
- **FR-004**: システムは旧MCP実装（`mcp-server/`）およびnpm公開導線を削除する必要がある。
- **FR-005**: システムはClaude Code向けプラグインを追加し、CLI利用スキルを提供する必要がある。
- **FR-006**: システムはCodex向けスキルを追加し、`.skill` パッケージ生成先を `codex-skills/dist` として案内する必要がある。
- **FR-007**: システムはREADME（日本語/英語）からnpm/npxインストール導線を削除し、CLI + plugin/skill導線に置換する必要がある。
- **FR-008**: システムはCI/Release設定からMCP npm publishジョブを除去する必要がある。

### 主要エンティティ
- **AssistantCurlRequest**: `command`, `auto_auth`, `timeout` を持つCLI入力。
- **AssistantCurlResult**: `success`, `status_code`, `body`, `error`, `duration_ms`, `executed_command` を持つCLI出力。
- **AssistantGuideCategory**: `overview`, `openai-compatible`, `endpoint-management`, `model-management`, `dashboard` の列挙。
- **ClaudePluginManifest**: Claude Codeプラグインのメタデータと参照スキル一覧。
- **CodexSkillPackageInfo**: Codexスキルの配置パスとパッケージ生成先。
