---
description: MCP Playwrightを使用してDashboardのE2Eテストを実行します。
---

## User Input

```text
$ARGUMENTS
```

You **MUST** consider the user input before proceeding (if not empty).

## Goal

MCP Playwrightツールを使用して、LLM Router DashboardのE2Eテストを対話的に実行する。
ブラウザを起動し、各UI要素の動作を検証し、スクリーンショットで結果を記録する。

## Prerequisites

- ルーターサーバーが `http://localhost:8080` で起動していること
- サーバーが起動していない場合は、別ターミナルで `cargo run --release -p llm-router` を実行

## Test Scenarios

### Phase 1: 初期表示テスト

1. **ナビゲーション**: `/dashboard` にアクセス
2. **初期スクリーンショット**: 初期状態を記録
3. **要素存在確認**:
   - ヘッダー (`header.page-header`)
   - Stats Grid (`.stats-grid`)
   - Models Section (`#tab-models`)
   - Nodes Table (`#nodes-body`)

### Phase 2: Header Controls テスト

| ID | テスト内容 | セレクタ | 操作 |
|---|----------|---------|------|
| H-01 | テーマ切替 | `#theme-toggle` | 3回クリック (minimal→tech→creative→minimal) |
| H-02 | Playgroundボタン | `#chat-open` | クリック→モーダル表示確認→閉じる |
| H-03 | API Keysボタン | `#api-keys-button` | クリック→モーダル表示確認→閉じる |
| H-04 | Refreshボタン | `#refresh-button` | クリック→ステータス更新確認 |
| H-05 | 接続ステータス | `#connection-status` | テキスト確認 |

### Phase 3: Stats Grid テスト

| ID | テスト内容 | セレクタ |
|---|----------|---------|
| S-01 | Total Nodes | `[data-stat="total-nodes"]` |
| S-02 | Online Nodes | `[data-stat="online-nodes"]` |
| S-03 | Offline Nodes | `[data-stat="offline-nodes"]` |
| S-04 | Total Requests | `[data-stat="total-requests"]` |
| S-05 | GPU Usage | `[data-stat="average-gpu-usage"]` |

### Phase 4: Models Tab テスト

| ID | テスト内容 | セレクタ | 操作 |
|---|----------|---------|------|
| M-01 | URL入力フィールド | `#hf-register-url` | 存在確認 |
| M-02 | 登録ボタン | `#hf-register-url-submit` | 存在確認 |
| M-03 | 登録済みモデル一覧 | `#registered-models-list` | 内容確認 |
| M-04 | Download Tasks | `#download-tasks-list` | 内容確認 |
| M-05 | Convert Tasks | `#convert-tasks-list` | 内容確認 |

### Phase 5: Nodes Tab テスト

| ID | テスト内容 | セレクタ | 操作 |
|---|----------|---------|------|
| N-01 | ノード一覧 | `#nodes-body` | 行数確認 |
| N-02 | ステータスフィルタ | `#filter-status` | 各オプション選択 |
| N-03 | 検索フィルタ | `#filter-query` | テキスト入力 |
| N-04 | Export JSON | `#export-json` | クリック |
| N-05 | Export CSV | `#export-csv` | クリック |

### Phase 6: Modals テスト

| ID | テスト内容 | セレクタ | 操作 |
|---|----------|---------|------|
| DM-01 | API Keysモーダル | `#api-keys-modal` | 開閉テスト |
| DM-02 | Convertモーダル | `#convert-modal` | 開閉テスト |
| DM-03 | Chatモーダル | `#chat-modal` | 開閉テスト |

## Execution Flow

各Phaseを順番に実行し、結果を報告する:

1. `mcp__playwright__playwright_navigate` でページを開く
2. `mcp__playwright__playwright_screenshot` で状態を記録
3. `mcp__playwright__playwright_click` で要素をクリック
4. `mcp__playwright__playwright_fill` でフォーム入力
5. `mcp__playwright__playwright_get_visible_text` でテキスト確認
6. `mcp__playwright__playwright_console_logs` でエラー確認

## Output Format

各テストの結果を以下の形式で報告:

```markdown
## E2E Test Results: Dashboard

### Summary
- Total Tests: X
- Passed: X
- Failed: X
- Skipped: X

### Phase 1: 初期表示
| ID | Status | Notes |
|----|--------|-------|
| ... | PASS/FAIL | ... |

### Phase 2: Header Controls
...

### Errors/Warnings
- Console errors found: ...
- Missing elements: ...

### Screenshots
- dashboard-initial.png
- dashboard-theme-tech.png
- ...
```

## Notes

- テスト中にエラーが発生した場合は、エラー内容を記録して次のテストに進む
- 要素が見つからない場合は `SKIP` として記録
- 最後にブラウザを閉じる (`mcp__playwright__playwright_close`)
