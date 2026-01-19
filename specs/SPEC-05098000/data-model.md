# データモデル: 推論中ノードへの多重リクエストキューイング

## エンティティ定義

### QueueEntry

```rust
/// キュー内の個別リクエストエントリ
pub struct QueueEntry {
    /// リクエスト一意識別子
    pub request_id: Uuid,
    /// ユーザー識別子（フェアネス制御用）
    pub user_id: Option<String>,
    /// 対象モデルID
    pub model_id: String,
    /// リクエスト到着時刻
    pub arrived_at: Instant,
    /// タイムアウト期限
    pub timeout_at: Instant,
    /// キャンセルトークン
    pub cancel_token: CancellationToken,
    /// 推論リクエスト本体
    pub request: InferenceRequest,
    /// レスポンス送信チャネル
    pub response_tx: oneshot::Sender<Result<InferenceResponse, QueueError>>,
}
```

### RequestQueue

```rust
/// ルーター側のグローバルリクエストキュー
pub struct RequestQueue {
    /// キューエントリ一覧
    entries: Arc<Mutex<VecDeque<QueueEntry>>>,
    /// 最大キューサイズ
    max_size: usize,
    /// キュー待機タイムアウト（秒）
    queue_timeout_secs: u64,
    /// エントリ追加通知
    notify: Arc<Notify>,
    /// 統計情報
    stats: Arc<RwLock<QueueStats>>,
}
```

### QueueStats

```rust
/// キュー統計情報
pub struct QueueStats {
    /// 現在の待機数
    pub waiting_count: usize,
    /// 処理中の数
    pub processing_count: usize,
    /// 過去1時間の平均待機時間（ミリ秒）
    pub avg_wait_time_ms: f64,
    /// 過去1時間の平均処理時間（ミリ秒）
    pub avg_process_time_ms: f64,
    /// 過去1時間のタイムアウト数
    pub timeout_count: u64,
    /// 過去1時間の429エラー数
    pub rejected_count: u64,
}
```

### NodeState

```rust
/// ノードの処理状態
#[derive(Clone, Copy, PartialEq)]
pub enum NodeState {
    /// アイドル状態（リクエスト受付可能）
    Idle,
    /// 推論処理中（新規リクエスト不可）
    Processing {
        /// 処理開始時刻
        started_at: Instant,
        /// 処理中のリクエストID
        request_id: Uuid,
    },
    /// モデルロード中（503を返す）
    Loading,
    /// オフライン
    Offline,
}
```

### QueueError

```rust
/// キュー関連エラー
pub enum QueueError {
    /// キューが満杯（HTTP 429）
    QueueFull {
        current_size: usize,
        retry_after_secs: u64,
    },
    /// キュー待機タイムアウト（HTTP 504）
    QueueTimeout {
        waited_secs: u64,
    },
    /// クライアント切断によるキャンセル
    ClientDisconnected,
    /// ノード障害
    NodeFailure {
        runtime_id: String,
        reason: String,
    },
}
```

### QueueConfig

```rust
/// キュー設定
pub struct QueueConfig {
    /// 最大キューサイズ（デフォルト: 100）
    pub max_queue_size: usize,
    /// キュー待機タイムアウト（秒、デフォルト: 60）
    pub queue_timeout_secs: u64,
    /// Retry-Afterのデフォルト値（秒）
    pub default_retry_after_secs: u64,
    /// フェアネス制御の有効化
    pub enable_fairness: bool,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 100,
            queue_timeout_secs: 60,
            default_retry_after_secs: 5,
            enable_fairness: true,
        }
    }
}
```

### EstimatedWait

```rust
/// 推定待ち時間
pub struct EstimatedWait {
    /// 推定待ち時間（秒）
    pub seconds: u64,
    /// キュー内の位置（1始まり）
    pub queue_position: usize,
    /// 推定精度（過去データ量による）
    pub confidence: EstimateConfidence,
}

pub enum EstimateConfidence {
    /// 十分なデータあり
    High,
    /// データ不足
    Low,
    /// 推定不可
    Unknown,
}
```

## 検証ルール

| エンティティ | フィールド | ルール |
|-------------|-----------|--------|
| QueueEntry | request_id | UUIDv4形式であること |
| QueueEntry | timeout_at | arrived_atより後であること |
| RequestQueue | max_size | 1以上であること |
| RequestQueue | queue_timeout_secs | 1以上であること |
| QueueConfig | max_queue_size | 1〜10000の範囲 |
| QueueConfig | queue_timeout_secs | 1〜3600の範囲 |
| QueueStats | waiting_count | entries.len()と一致すること |

## 関係図

```text
┌─────────────────────────────────────────────────────────────────┐
│                         Router                                   │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    RequestQueue                            │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐         │  │
│  │  │ Entry 1 │→│ Entry 2 │→│ Entry 3 │→│ ...     │         │  │
│  │  └────┬────┘ └────┬────┘ └────┬────┘ └─────────┘         │  │
│  │       │           │           │                           │  │
│  │       ▼           ▼           ▼                           │  │
│  │   user_id     user_id     user_id                         │  │
│  │   model_id    model_id    model_id                        │  │
│  │   timeout_at  timeout_at  timeout_at                      │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│                              ▼ dispatch                          │
│  ┌─────────────────┬─────────────────┬─────────────────┐        │
│  │    Node A       │    Node B       │    Node C       │        │
│  │ state: Idle     │ state: Process  │ state: Idle     │        │
│  │ queue: 0        │ queue: 0        │ queue: 0        │        │
│  └─────────────────┴─────────────────┴─────────────────┘        │
└─────────────────────────────────────────────────────────────────┘

リクエストフロー:
1. Client → Router: リクエスト到着
2. Router: ノード選択（アイドル優先）
   - アイドルノードあり → 即座にディスパッチ
   - 全ノード処理中 → キューに追加
3. Router → Node: 単発リクエスト送信
4. Node: 推論実行（state: Processing）
5. Node → Router: レスポンス返却
6. Router → Client: レスポンス転送
```

## HTTPレスポンスヘッダー

| ヘッダー | 説明 | 例 |
|---------|------|-----|
| X-Queue-Position | キュー内の位置 | `X-Queue-Position: 3` |
| X-Estimated-Wait | 推定待ち時間（秒） | `X-Estimated-Wait: 30` |
| Retry-After | 再試行までの推奨時間（秒） | `Retry-After: 5` |
