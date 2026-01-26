# 技術リサーチ: 統一APIプロキシ

## リサーチ課題

1. OpenAI API互換プロキシの実装方法
2. ストリーミングレスポンスの透過的転送
3. プロキシオーバーヘッドの最小化
4. 同時リクエスト処理の設計

## 1. OpenAI API互換プロキシ

### 決定

**axum + reqwest による非同期HTTPプロキシ**

### 理由

- axumは高性能な非同期Webフレームワーク
- reqwestはHTTP/2、接続プール、ストリーミングをサポート
- Rustの所有権システムによりメモリ安全性を保証

### 代替案比較表

| フレームワーク | パフォーマンス | エコシステム | 学習曲線 | 採用 |
|---------------|---------------|--------------|---------|------|
| axum | 最高 | 豊富 | 中 | ✅ |
| actix-web | 最高 | 豊富 | 高 | × |
| warp | 高 | 中 | 高 | × |
| hyper直接 | 最高 | 低 | 最高 | × |

### サポートエンドポイント

| エンドポイント | メソッド | 機能 |
|---------------|---------|------|
| `/v1/chat/completions` | POST | チャット完了 |
| `/v1/completions` | POST | テキスト生成 |
| `/v1/embeddings` | POST | 埋め込み生成 |
| `/v1/models` | GET | モデル一覧 |
| `/v1/models/{id}` | GET | モデル詳細 |

## 2. ストリーミングレスポンス

### 決定

**Server-Sent Events (SSE) パススルー**

### 理由

- OpenAI APIはSSE形式でストリーミング
- クライアントへの透過的な転送が必須
- バッファリングなしでリアルタイム配信

### 実装方法

```text
[Client] ←SSE→ [Router] ←SSE→ [Node]

1. Load Balancer はノードからのチャンク受信を即座に転送
2. Content-Type: text/event-stream を維持
3. Transfer-Encoding: chunked を使用
4. 接続が切れた場合は適切にクリーンアップ
```

### ストリーミングフロー

```text
Client                Load Balancer                Node
  |                     |                     |
  |-- POST stream=true→ |                     |
  |                     |-- POST stream=true→ |
  |                     |                     |
  |                     | ←data: chunk1       |
  | ←data: chunk1       |                     |
  |                     | ←data: chunk2       |
  | ←data: chunk2       |                     |
  |                     | ←data: [DONE]       |
  | ←data: [DONE]       |                     |
```

## 3. プロキシオーバーヘッド最小化

### 決定

**ゼロコピー転送 + 接続プール**

### 理由

- LLM推論は秒単位の処理時間
- プロキシ部分は50ms以内必須
- メモリコピーを最小化

### 最適化手法

| 手法 | 効果 | 実装コスト | 採用 |
|------|------|-----------|------|
| 接続プール | 高 | 低 | ✅ |
| ゼロコピー転送 | 高 | 中 | ✅ |
| HTTP/2多重化 | 中 | 低 | ✅ |
| Keep-Alive | 高 | 低 | ✅ |

### パフォーマンス目標

```text
プロキシ処理時間内訳:
- ノード選択: ~1ms
- リクエスト転送開始: ~5ms
- レスポンス受信開始: ノード処理時間
- オーバーヘッド合計: <50ms
```

## 4. 同時リクエスト処理

### 決定

**tokioベースの非同期処理 + セマフォ制限**

### 理由

- 100同時リクエスト処理が要件
- 各リクエストは独立して非同期処理
- リソース枯渇防止にセマフォを使用

### 実装方法

```rust
// 同時リクエスト数制限
const MAX_CONCURRENT_REQUESTS: usize = 100;

pub struct ProxyService {
    semaphore: Arc<Semaphore>,
    client: reqwest::Client,
}

impl ProxyService {
    pub async fn proxy(&self, req: Request) -> Result<Response> {
        let _permit = self.semaphore.acquire().await?;
        // プロキシ処理
    }
}
```

### スケーリング設計

```text
                    ┌─────────────┐
                    │   Load Balancer    │
                    │ (100 slots) │
                    └──────┬──────┘
           ┌───────────────┼───────────────┐
           ▼               ▼               ▼
      ┌────────┐      ┌────────┐      ┌────────┐
      │ Node 1 │      │ Node 2 │      │ Node 3 │
      └────────┘      └────────┘      └────────┘

- 各ノードへの接続は接続プールで管理
- ノード障害時は他ノードへフェイルオーバー
- すべてのノードが応答不能なら503を返却
```

## 参考リソース

- [OpenAI API Reference](https://platform.openai.com/docs/api-reference)
- [axum Documentation](https://docs.rs/axum/)
- [reqwest Streaming](https://docs.rs/reqwest/latest/reqwest/struct.Response.html#method.bytes_stream)
- [Server-Sent Events Specification](https://html.spec.whatwg.org/multipage/server-sent-events.html)
