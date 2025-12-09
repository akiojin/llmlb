---
description: MCP Playwrightを使用してPlaygroundのE2Eテストを実行します。
---

## User Input

```text
$ARGUMENTS
```

You **MUST** consider the user input before proceeding (if not empty).

## Goal

MCP Playwrightツールを使用して、LLM Router PlaygroundのE2Eテストを対話的に実行する。
ブラウザを起動し、チャット機能・設定・セッション管理の動作を検証する。

## Prerequisites

- ルーターサーバーが `http://localhost:8080` で起動していること
- サーバーが起動していない場合は、別ターミナルで `cargo run --release -p llm-router` を実行

## Test Scenarios

### Phase 1: 初期表示テスト

1. **ナビゲーション**: `/playground` にアクセス
2. **初期スクリーンショット**: 初期状態を記録
3. **要素存在確認**:
   - サイドバー (`#sidebar`)
   - チャットエリア (`.chat-main`)
   - モデル選択 (`#model-select`)
   - 入力フォーム (`#chat-form`)

### Phase 2: Sidebar テスト

| ID | テスト内容 | セレクタ | 操作 |
|---|----------|---------|------|
| PS-01 | サイドバー表示 | `#sidebar` | 存在確認 |
| PS-02 | 折りたたみトグル | `#sidebar-toggle` | クリック→状態変化確認 |
| PS-03 | New Playgroundボタン | `#new-chat` | クリック→新規セッション作成 |
| PS-04 | セッションリスト | `#session-list` | 項目確認 |

### Phase 3: Header テスト

| ID | テスト内容 | セレクタ | 操作 |
|---|----------|---------|------|
| PH-01 | モデル選択 | `#model-select` | オプション一覧確認 |
| PH-02 | 接続ステータス | `#router-status` | 状態確認 |
| PH-03 | 設定ボタン | `#settings-toggle` | クリック→モーダル表示 |

### Phase 4: Chat Input テスト

| ID | テスト内容 | セレクタ | 操作 |
|---|----------|---------|------|
| PC-01 | メッセージ入力 | `#chat-input` | テキスト入力 |
| PC-02 | 送信ボタン | `#send-button` | 存在確認 |
| PC-03 | 停止ボタン | `#stop-button` | 存在確認（hidden状態） |
| PC-04 | Enterキー送信 | `#chat-input` | Enter押下 |

### Phase 5: Settings Modal テスト

| ID | テスト内容 | セレクタ | 操作 |
|---|----------|---------|------|
| PST-01 | モーダル開く | `#settings-toggle` | クリック |
| PST-02 | Provider切替 | `.provider-btn` | 各ボタンクリック |
| PST-03 | API Key入力 | `#api-key-input` | テキスト入力 |
| PST-04 | ストリーミングトグル | `#stream-toggle` | チェック切替 |
| PST-05 | システムプロンプト | `#system-prompt` | テキスト入力 |
| PST-06 | Clear Playground | `#reset-chat` | クリック |
| PST-07 | Copy cURL | `#copy-curl` | クリック |
| PST-08 | モーダル閉じる | `#modal-close` | クリック |

### Phase 6: Chat Flow テスト（オプション）

モデルが利用可能な場合のみ実行:

| ID | テスト内容 | 操作 |
|---|----------|------|
| CF-01 | メッセージ送信 | テキスト入力→送信 |
| CF-02 | ユーザーメッセージ表示 | `.message--user` 確認 |
| CF-03 | レスポンス待機 | `.message--assistant` 確認 |
| CF-04 | 履歴保存 | セッションリスト更新確認 |

## Execution Flow

各Phaseを順番に実行し、結果を報告する:

1. `mcp__playwright__playwright_navigate` でページを開く
2. `mcp__playwright__playwright_screenshot` で状態を記録
3. `mcp__playwright__playwright_click` で要素をクリック
4. `mcp__playwright__playwright_fill` でフォーム入力
5. `mcp__playwright__playwright_press_key` でキーボード操作
6. `mcp__playwright__playwright_get_visible_text` でテキスト確認
7. `mcp__playwright__playwright_console_logs` でエラー確認

## Output Format

各テストの結果を以下の形式で報告:

```markdown
## E2E Test Results: Playground

### Summary
- Total Tests: X
- Passed: X
- Failed: X
- Skipped: X

### Phase 1: 初期表示
| ID | Status | Notes |
|----|--------|-------|
| ... | PASS/FAIL | ... |

### Phase 2: Sidebar
...

### Errors/Warnings
- Console errors found: ...
- Missing elements: ...

### Screenshots
- playground-initial.png
- playground-settings-open.png
- ...
```

## Notes

- モデルが利用できない場合、Phase 6 はスキップ
- テスト中にエラーが発生した場合は、エラー内容を記録して次のテストに進む
- 最後にブラウザを閉じる (`mcp__playwright__playwright_close`)
