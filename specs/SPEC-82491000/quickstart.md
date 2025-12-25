# クイックスタート: クラウドプロバイダーモデル一覧統合

**機能ID**: `SPEC-82491000` | **日付**: 2025-12-25

## 前提条件

- llm-routerが起動していること
- 以下の環境変数が設定されていること（使用したいプロバイダーのみ）:
  - `OPENAI_API_KEY`: OpenAI APIキー
  - `GOOGLE_API_KEY`: Google AI APIキー
  - `ANTHROPIC_API_KEY`: Anthropic APIキー

## 基本的な使い方

### モデル一覧の取得

```bash
curl -X GET http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_debug"
```

**レスポンス例**:

```json
{
  "object": "list",
  "data": [
    {
      "id": "llama-3.2",
      "object": "model",
      "created": 0,
      "owned_by": "router",
      "ready": true
    },
    {
      "id": "openai:gpt-4o",
      "object": "model",
      "created": 1686935002,
      "owned_by": "openai"
    },
    {
      "id": "google:gemini-2.0-flash",
      "object": "model",
      "created": 0,
      "owned_by": "google"
    },
    {
      "id": "anthropic:claude-sonnet-4-20250514",
      "object": "model",
      "created": 1715644800,
      "owned_by": "anthropic"
    }
  ]
}
```

## 検証シナリオ

### シナリオ1: 全プロバイダー設定時

```bash
# 環境変数設定
export OPENAI_API_KEY="sk-..."
export GOOGLE_API_KEY="..."
export ANTHROPIC_API_KEY="sk-ant-..."

# モデル一覧取得
curl -s http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_debug" | jq '.data[] | .id' | head -10
```

**期待される出力**:

```text
"llama-3.2"
"openai:gpt-4o"
"openai:gpt-4o-mini"
"google:gemini-2.0-flash"
"google:gemini-1.5-pro"
"anthropic:claude-sonnet-4-20250514"
"anthropic:claude-3-5-sonnet-20241022"
...
```

### シナリオ2: 一部プロバイダーのみ設定時

```bash
# OpenAIのみ設定
export OPENAI_API_KEY="sk-..."
unset GOOGLE_API_KEY
unset ANTHROPIC_API_KEY

# モデル一覧取得
curl -s http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_debug" | jq '.data[] | select(.owned_by == "openai") | .id'
```

**期待される出力**:

```text
"openai:gpt-4o"
"openai:gpt-4o-mini"
...
```

### シナリオ3: キャッシュ動作確認

```bash
# 1回目のリクエスト（キャッシュミス）
time curl -s http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_debug" > /dev/null

# 2回目のリクエスト（キャッシュヒット - 高速）
time curl -s http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_debug" > /dev/null
```

**期待される結果**:

- 1回目: 1-3秒程度（API呼び出し含む）
- 2回目: 100ms未満（キャッシュから）

### シナリオ4: プロバイダー障害時

```bash
# 無効なAPIキーを設定（認証エラーをシミュレート）
export OPENAI_API_KEY="invalid-key"
export GOOGLE_API_KEY="valid-key"

# モデル一覧取得 - OpenAI以外は取得できる
curl -s http://localhost:8080/v1/models \
  -H "Authorization: Bearer sk_debug" | jq '.data[] | .owned_by' | sort | uniq
```

**期待される出力**:

```text
"google"
"router"
```

（OpenAIモデルは表示されないが、エラーにはならない）

## トラブルシューティング

### クラウドモデルが表示されない

1. 環境変数が正しく設定されているか確認:

   ```bash
   echo $OPENAI_API_KEY
   echo $GOOGLE_API_KEY
   echo $ANTHROPIC_API_KEY
   ```

2. APIキーが有効か確認（直接API呼び出し）:

   ```bash
   curl https://api.openai.com/v1/models \
     -H "Authorization: Bearer $OPENAI_API_KEY" | head -c 200
   ```

### キャッシュをクリアしたい

現在、キャッシュクリアAPIは提供されていません。
ルーターを再起動するとキャッシュがクリアされます。

---

*Phase 1 クイックスタート完了*
