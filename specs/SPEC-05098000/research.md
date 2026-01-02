# 技術リサーチ: 推論中ノードへの多重リクエストキューイング

## リサーチ課題

1. ルーター側キューイングの最適な実装パターン
2. フェアネス制御のアルゴリズム選択
3. タイムアウト・キャンセル処理の設計

## 1. キューイング実装パターン

### 決定

**Tokio mpsc channel + Arc<Mutex<VecDeque>>** のハイブリッド方式を採用

### 理由

- Tokioの非同期ランタイムとシームレスに統合
- VecDequeによりO(1)のenqueue/dequeue操作を実現
- Arc<Mutex>によりマルチスレッド安全性を確保

### 代替案比較表

| 方式 | 長所 | 短所 | 判定 |
|------|------|------|------|
| tokio::sync::mpsc | Tokioネイティブ、backpressure対応 | キュー内操作が困難 | △ |
| crossbeam-queue | ロックフリー、高スループット | asyncとの統合が複雑 | × |
| VecDeque + Mutex | シンプル、柔軟なキュー操作 | ロック競合の可能性 | ○ |
| flume | 高性能、backpressure | 外部依存追加 | △ |

### 実装方法

```rust
// キューエントリ
struct QueueEntry {
    request_id: Uuid,
    user_id: Option<String>,
    arrived_at: Instant,
    timeout_at: Instant,
    request: InferenceRequest,
    response_tx: oneshot::Sender<Result<InferenceResponse>>,
}

// グローバルキュー
struct RequestQueue {
    entries: Arc<Mutex<VecDeque<QueueEntry>>>,
    max_size: usize,
    notify: Arc<Notify>,
}
```

## 2. フェアネス制御アルゴリズム

### 決定

**Weighted Fair Queueing (WFQ)** のシンプル版（ラウンドロビン）を採用

### 理由

- ユーザー間の公平性を保証
- 実装がシンプル
- 特定ユーザーによるキュー独占を防止

### 代替案比較表

| アルゴリズム | 公平性 | 実装難易度 | オーバーヘッド | 判定 |
|-------------|--------|-----------|--------------|------|
| FIFO（先着順） | 低 | 簡単 | 最小 | × |
| ラウンドロビン | 高 | 簡単 | 低 | ○ |
| WFQ（重み付き） | 最高 | 中程度 | 中 | △ |
| Token Bucket | 高 | 複雑 | 高 | × |

### 実装方法

```rust
// ユーザー別にキューをグループ化
struct FairQueue {
    // user_id -> pending requests
    user_queues: HashMap<String, VecDeque<QueueEntry>>,
    // 次にサービスするユーザーのインデックス
    current_user_idx: usize,
    // ユーザー順序（ラウンドロビン用）
    user_order: Vec<String>,
}
```

## 3. タイムアウト・キャンセル処理

### 決定

**tokio::select! + CancellationToken** パターンを採用

### 理由

- Tokioの非同期キャンセルパターンに準拠
- クライアント切断の即座検知が可能
- リソースリークを防止

### 代替案比較表

| 方式 | 即座性 | リソース管理 | 複雑さ | 判定 |
|------|--------|-------------|--------|------|
| 定期ポーリング | 低 | 中 | 低 | × |
| tokio::time::timeout | 高 | 良 | 低 | △ |
| select! + CancellationToken | 最高 | 最良 | 中 | ○ |
| abortable future | 高 | 良 | 中 | △ |

### 実装方法

```rust
async fn process_request(entry: QueueEntry) -> Result<()> {
    let timeout = tokio::time::sleep_until(entry.timeout_at);
    let cancel_token = entry.cancel_token.clone();

    tokio::select! {
        result = execute_inference(&entry.request) => {
            entry.response_tx.send(result).ok();
        }
        _ = timeout => {
            entry.response_tx.send(Err(QueueTimeout)).ok();
        }
        _ = cancel_token.cancelled() => {
            // クライアント切断、リソース解放のみ
        }
    }
}
```

## 4. ノード選択とキューイングの統合

### 決定

キュー長重み付きロードバランシング（スコア = GPU負荷×0.3 + キュー長×0.7）

### 理由

- キュー長を重視することで待機時間を最小化
- GPU負荷も考慮してノード過負荷を防止
- SPEC-589f2df1のロードバランサーと統合しやすい

### 実装方法

```rust
fn select_node(nodes: &[Node], model_id: &str) -> Option<&Node> {
    nodes
        .iter()
        .filter(|n| n.supports_model(model_id) && n.is_online())
        .min_by_key(|n| {
            let gpu_score = (n.gpu_usage * 0.3 * 100.0) as u32;
            let queue_score = (n.queue_length as f32 * 0.7 * 10.0) as u32;
            gpu_score + queue_score
        })
}
```

## 参考リソース

- [Tokio Tutorial - Channels](https://tokio.rs/tokio/tutorial/channels)
- [Fair Queueing Algorithm (RFC 970)](https://datatracker.ietf.org/doc/html/rfc970)
- [Axum Request Cancellation](https://docs.rs/axum/latest/axum/#extractors)
- [OpenAI API Rate Limits](https://platform.openai.com/docs/guides/rate-limits)
