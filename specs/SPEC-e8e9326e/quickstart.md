# クイックスタート: ロードバランサー主導エンドポイント登録システム

**機能ID**: `SPEC-e8e9326e`
**日付**: 2026-01-14

## 概要

ロードバランサー主導エンドポイント登録システムを使用して、推論エンドポイントを管理する方法を説明します。

## 前提条件

- ロードバランサーが起動済み（`http://localhost:32768`）
- 管理者アカウントでログイン済み、またはAPIキーを取得済み
- 登録対象のエンドポイント（Ollama、自社ノード等）が稼働中

## ダッシュボードからのエンドポイント登録

### ステップ1: エンドポイント一覧画面を開く

1. ダッシュボード（`http://localhost:32768/dashboard`）にログイン
2. サイドメニューから「エンドポイント」を選択

### ステップ2: 新規エンドポイントを登録

1. 「新規エンドポイント」ボタンをクリック
2. 以下の情報を入力:
   - **名前**: 識別しやすい名前（例: "本番Ollama"）
   - **URL**: エンドポイントのベースURL（例: `http://192.168.1.100:11434`）
   - **APIキー**: 必要な場合のみ入力
3. 「接続テスト」ボタンで接続を確認
4. 「保存」ボタンで登録完了

### ステップ3: 状態確認

- 登録直後は「保留中」状態
- ヘルスチェック成功後、自動的に「オンライン」に遷移
- エンドポイント一覧で状態を確認可能
- ステータスバッジは状態別に色分けされる（online=緑、pending=黄、offline=赤系淡色、error=赤）

## REST APIからのエンドポイント登録

### エンドポイント登録

```bash
curl -X POST http://localhost:32768/api/endpoints \
  -H "Authorization: Bearer sk_your_api_key" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "本番Ollama",
    "base_url": "http://192.168.1.100:11434"
  }'
```

**レスポンス例**:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "本番Ollama",
  "base_url": "http://192.168.1.100:11434",
  "status": "pending",
  "health_check_interval_secs": 30,
  "registered_at": "2026-01-14T10:30:00Z"
}
```

### エンドポイント一覧取得

```bash
curl http://localhost:32768/api/endpoints \
  -H "Authorization: Bearer sk_your_api_key"
```

### エンドポイント詳細取得

```bash
curl http://localhost:32768/api/endpoints/{endpoint_id} \
  -H "Authorization: Bearer sk_your_api_key"
```

### 接続テスト

```bash
curl -X POST http://localhost:32768/api/endpoints/{endpoint_id}/test \
  -H "Authorization: Bearer sk_your_api_key"
```

### モデル同期

```bash
curl -X POST http://localhost:32768/api/endpoints/{endpoint_id}/sync \
  -H "Authorization: Bearer sk_your_api_key"
```

## エンドポイント登録例

すべてのエンドポイントはOpenAI互換APIとして統一的に扱われます。

### xLLM（自社推論サーバー）

```json
{
  "name": "開発xLLM1",
  "base_url": "http://192.168.1.50:32768"
}
```

### Ollama

```json
{
  "name": "OllamaサーバーA",
  "base_url": "http://192.168.1.100:11434"
}
```

### vLLM

```json
{
  "name": "vLLMサーバー",
  "base_url": "http://192.168.1.200:8000"
}
```

### APIキーが必要なエンドポイント

```json
{
  "name": "外部APIサービス",
  "base_url": "https://api.example.com",
  "api_key": "sk-xxx..."
}
```

## 検証ステップ

### 1. エンドポイント登録の検証

```bash
# 登録
curl -X POST http://localhost:32768/api/endpoints \
  -H "Authorization: Bearer sk_debug" \
  -H "Content-Type: application/json" \
  -d '{"name": "Test", "base_url": "http://localhost:11434"}'

# 確認
curl http://localhost:32768/api/endpoints \
  -H "Authorization: Bearer sk_debug"
```

**期待結果**: エンドポイントが一覧に表示される

### 2. ヘルスチェックの検証

```bash
# 30秒待機後、状態確認
curl http://localhost:32768/api/endpoints/{endpoint_id} \
  -H "Authorization: Bearer sk_debug"
```

**期待結果**: `status` が `online` に変わる（エンドポイントが稼働中の場合）

### 2.1 ダッシュボードのステータス色分け検証

1. ダッシュボードの Endpoints 一覧を開く
2. 各エンドポイントのステータス表示を確認する（詳細モーダル、Playgroundでも確認）

**期待結果**:
- `online` は緑系表示
- `pending` は黄系表示
- `offline` は赤系淡色表示
- `error` は赤表示

### 2.2 ダッシュボードのTPSソート検証

1. ダッシュボードの Endpoints 一覧で `TPS` 列ヘッダーをクリックする（1回目: 降順）
2. `aggregate_tps` が高い行ほど上位になることを確認する
3. `TPS` 列ヘッダーをもう一度クリックする（2回目: 昇順）
4. 未計測TPS（`—`）の行が昇順/降順どちらでも常に末尾に残ることを確認する

**期待結果**:
- 1回目クリックで降順、2回目クリックで昇順に切り替わる
- `aggregate_tps = null` の行は常に最下部に配置される

### 3. モデル同期の検証

```bash
# 同期実行
curl -X POST http://localhost:32768/api/endpoints/{endpoint_id}/sync \
  -H "Authorization: Bearer sk_debug"

# モデル一覧確認
curl http://localhost:32768/v1/models \
  -H "Authorization: Bearer sk_debug"
```

**期待結果**: エンドポイントのモデルがモデル一覧に表示される

### 4. 推論リクエストの検証

```bash
curl -X POST http://localhost:32768/v1/chat/completions \
  -H "Authorization: Bearer sk_debug" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3.1:8b",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

**期待結果**: エンドポイントにルーティングされ、レスポンスが返る

### 5. エンドポイントタイプ自動判別の検証

エンドポイント登録時に自動的にタイプが判別されます。

```bash
# エンドポイント登録（タイプ自動判別）
curl -X POST http://localhost:32768/api/endpoints \
  -H "Authorization: Bearer sk_debug" \
  -H "Content-Type: application/json" \
  -d '{"name": "xLLM Server", "base_url": "http://localhost:8080"}'
```

**期待結果**: レスポンスに `endpoint_type` フィールドが含まれる

```json
{
  "id": "...",
  "name": "xLLM Server",
  "endpoint_type": "xllm",
  ...
}
```

判別優先度:

1. **xllm**: GET /api/system に `xllm_version` が含まれる
2. **ollama**: GET /api/tags が成功
3. **vllm**: Server ヘッダーに "vllm" が含まれる
4. **openai_compatible**: GET /v1/models が成功
5. **unknown**: 判別不能（オフライン時）

### 6. タイプフィルタリングの検証

```bash
# xLLMタイプのみ取得
curl "http://localhost:32768/api/endpoints?type=xllm" \
  -H "Authorization: Bearer sk_debug"

# Ollamaタイプのみ取得
curl "http://localhost:32768/api/endpoints?type=ollama" \
  -H "Authorization: Bearer sk_debug"
```

**期待結果**: 指定タイプのエンドポイントのみがフィルタリングされる

### 7. xLLMモデルダウンロードの検証（xLLMタイプのみ）

```bash
# ダウンロード開始
curl -X POST http://localhost:32768/api/endpoints/{endpoint_id}/download \
  -H "Authorization: Bearer sk_debug" \
  -H "Content-Type: application/json" \
  -d '{"model": "llama-3.2-1b"}'

# 進捗確認
curl "http://localhost:32768/api/endpoints/{endpoint_id}/download/progress?model=llama-3.2-1b" \
  -H "Authorization: Bearer sk_debug"
```

**期待結果**:

- xLLMタイプ: ダウンロードが開始される
- 非xLLMタイプ: 400 Bad Request（ダウンロード非対応）

### 8. モデルメタデータ取得の検証（xLLM/Ollamaのみ）

```bash
# モデル情報取得
curl http://localhost:32768/api/endpoints/{endpoint_id}/models/{model_id}/info \
  -H "Authorization: Bearer sk_debug"
```

**期待結果**:

```json
{
  "model": "llama-3.2-1b",
  "context_length": 131072,
  "capabilities": ["text", "vision"]
}
```

## トラブルシューティング

### エンドポイントがオフラインのまま

1. エンドポイントのURLが正しいか確認
2. エンドポイントが実際に稼働中か確認
3. ファイアウォール設定を確認
4. 接続テストでエラー詳細を確認

### モデルが同期されない

1. エンドポイントがオンライン状態か確認
2. 手動で同期を実行
3. エンドポイントのモデル一覧APIが正常に動作するか確認

### 認証エラー

1. APIキーが正しいか確認
2. APIキーの権限を確認
3. JWTトークンの有効期限を確認

---

*クイックスタートガイド完了*
