---
description: MCP Playwrightを使用してDashboardとPlaygroundの完全E2Eテストを実行します。
---

## User Input

```text
$ARGUMENTS
```

You **MUST** consider the user input before proceeding (if not empty).

## Goal

MCP Playwrightツールを使用して、LLM RouterのDashboardとPlayground両方の完全なE2Eテストを実行する。
すべてのUI要素、インタラクション、モーダル、フォームを網羅的にテストする。

## Prerequisites

- ルーターサーバーが `http://localhost:8080` で起動していること
- サーバーが起動していない場合は、別ターミナルで `cargo run --release -p llm-router` を実行

## Test Execution Order

### Part 1: Dashboard Tests

Execute `/e2e.dashboard` scenarios:

1. 初期表示テスト
2. Header Controls (テーマ、Playground、API Keys、Refresh)
3. Stats Grid (ノード数、リクエスト数、GPU使用率)
4. Models Tab (URL登録、モデル一覧、タスク)
5. Nodes Tab (フィルタ、ソート、ページネーション、Export)
6. Modals (API Keys、Convert、Chat)

### Part 2: Playground Tests

Execute `/e2e.playground` scenarios:

1. 初期表示テスト
2. Sidebar (折りたたみ、セッション管理)
3. Header (モデル選択、ステータス、設定)
4. Chat Input (入力、送信)
5. Settings Modal (Provider、API Key、ストリーミング)
6. Chat Flow (メッセージ送受信)

## Full Test Matrix

### Dashboard (90+ checkpoints)

| Category | Tests | Selectors |
|----------|-------|-----------|
| Header | 7 | `#theme-toggle`, `#chat-open`, `#api-keys-button`, `#refresh-button`, `#connection-status`, `#last-refreshed`, `#refresh-metrics` |
| Stats | 8 | `[data-stat="*"]` variants |
| Models | 8 | `#hf-register-url`, `#hf-register-url-submit`, `#registered-models-list`, `#download-tasks-list`, `#convert-tasks-list`, `#convert-modal` |
| Nodes | 13 | `#nodes-body`, `#filter-status`, `#filter-query`, `th[data-sort="*"]`, `#page-prev`, `#page-next`, `#select-all`, `#export-json`, `#export-csv` |
| History | 7 | `#request-history-tbody`, `#filter-history-model`, `#history-per-page`, `#history-page-prev`, `#history-page-next`, `#export-history-csv` |
| Logs | 5 | `#logs-coordinator-list`, `#logs-coordinator-refresh`, `#logs-node-select`, `#logs-node-refresh` |
| Modals | 13 | `#node-modal`, `#request-modal`, `#api-keys-modal`, `#chat-modal`, `#convert-modal` |

### Playground (29+ checkpoints)

| Category | Tests | Selectors |
|----------|-------|-----------|
| Sidebar | 6 | `#sidebar`, `#sidebar-toggle`, `#new-chat`, `#session-list` |
| Chat | 12 | `#model-select`, `#chat-input`, `#send-button`, `#stop-button`, `.message--user`, `.message--assistant`, `#router-status`, `#error-banner` |
| Settings | 11 | `#settings-toggle`, `#modal-close`, `.provider-btn`, `#api-key-input`, `#stream-toggle`, `#append-system`, `#system-prompt`, `#reset-chat`, `#copy-curl` |

## Execution Flow

```
1. Navigate to /dashboard
   ├── Take initial screenshot
   ├── Execute Header tests
   ├── Execute Stats tests
   ├── Execute Models tests
   ├── Execute Nodes tests
   ├── Execute Modals tests
   └── Take final screenshot

2. Navigate to /playground
   ├── Take initial screenshot
   ├── Execute Sidebar tests
   ├── Execute Header tests
   ├── Execute Chat Input tests
   ├── Execute Settings tests
   ├── Execute Chat Flow tests (if model available)
   └── Take final screenshot

3. Generate summary report
4. Close browser
```

## MCP Playwright Commands Used

- `mcp__playwright__playwright_navigate` - ページ遷移
- `mcp__playwright__playwright_screenshot` - スクリーンショット
- `mcp__playwright__playwright_click` - クリック
- `mcp__playwright__playwright_fill` - フォーム入力
- `mcp__playwright__playwright_select` - セレクト選択
- `mcp__playwright__playwright_hover` - ホバー
- `mcp__playwright__playwright_press_key` - キー押下
- `mcp__playwright__playwright_get_visible_text` - テキスト取得
- `mcp__playwright__playwright_get_visible_html` - HTML取得
- `mcp__playwright__playwright_console_logs` - コンソールログ
- `mcp__playwright__playwright_evaluate` - JavaScript実行
- `mcp__playwright__playwright_close` - ブラウザ終了

## Output Format

```markdown
# Full E2E Test Report

## Executive Summary
- **Date**: YYYY-MM-DD HH:MM
- **Duration**: X minutes
- **Total Tests**: X
- **Passed**: X (X%)
- **Failed**: X
- **Skipped**: X

## Dashboard Results

### Header Controls
| ID | Test | Status | Notes |
|----|------|--------|-------|
| H-01 | Theme Toggle | PASS | Cycled through 3 themes |
| ... | ... | ... | ... |

### Stats Grid
...

### Models Tab
...

### Nodes Tab
...

### Modals
...

## Playground Results

### Sidebar
...

### Chat
...

### Settings
...

## Console Errors
- [list any JavaScript errors]

## Failed Tests Details
- [detailed info for each failure]

## Screenshots
- dashboard-initial.png
- dashboard-final.png
- playground-initial.png
- playground-final.png

## Recommendations
- [any issues found that need attention]
```

## Error Handling

- 要素が見つからない場合: `SKIP` として記録し、次のテストへ
- タイムアウト: 10秒待機後にタイムアウトとして記録
- JavaScript エラー: 全テスト完了後にまとめて報告
- ネットワークエラー: 即座に報告し、テスト継続判断を促す
