# Data Model: SPEC-08af4e3c

## 1. AssistantCurlRequest
- `command: String` - 実行対象のcurlコマンド。
- `auto_auth: bool` - 認証ヘッダー自動注入の有効/無効。
- `timeout: u64` - タイムアウト秒（1-300）。

## 2. AssistantCurlResult
- `success: bool` - HTTP 2xxかどうか。
- `status_code: Option<u16>` - HTTPステータスコード。
- `body: Option<serde_json::Value or String>` - レスポンスボディ。
- `error: Option<String>` - 失敗理由。
- `duration_ms: u128` - 実行時間。
- `executed_command: String` - マスキング後コマンド。

## 3. AssistantGuideCategory
- `overview`
- `openai-compatible`
- `endpoint-management`
- `model-management`
- `dashboard`

## 4. Plugin/Skill Metadata
### Claude Plugin
- `name`
- `version`
- `description`
- `skills[]`

### Codex Skill
- `name`
- `description`
- `allowed-tools`

## 5. Security Rules
- 禁止オプション集合（`-o`, `--output`, `-K`, `--config`, `-u`, `--user`, など）。
- 禁止パターン集合（`;`, `|`, `` ` ``, `$(`, `${`, リダイレクト）。
- 許可ホスト集合（`localhost`, `127.0.0.1`, `::1`, および `LLMLB_URL` のhost:port）。
