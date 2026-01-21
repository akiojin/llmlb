# 技術リサーチ: ルーター負荷最適化

## リサーチ課題

1. HTTPクライアントプーリングの設計
2. 待機機構の改善（タイムアウト・バックプレッシャー）
3. ノード選択の最適化（キャッシュ）
4. メモリ効率とパフォーマンス計測

## 1. HTTPクライアントプーリング

### 決定

**reqwest の接続プール**を活用（追加の接続プールライブラリは不要）

### 理由

- reqwest は内部で hyper の接続プールを使用
- keep-alive 接続のデフォルトサポート
- 設定変更のみで最適化可能

### 代替案比較

| 方式 | 説明 | メリット | デメリット |
|------|------|---------|-----------|
| reqwest 標準 | 内蔵接続プール | シンプル、依存なし | カスタマイズ制限 |
| bb8/deadpool | 汎用接続プール | 高度な制御 | 複雑性増加 |
| カスタム実装 | 独自プール | 完全制御 | 実装コスト大 |

### 実装方法

```rust
// router/src/proxy/client.rs

use reqwest::Client;
use std::time::Duration;

/// 最適化された HTTP クライアントを生成
pub fn create_optimized_client() -> Client {
    Client::builder()
        // 接続プール設定
        .pool_max_idle_per_host(32)        // ホストあたり最大アイドル接続数
        .pool_idle_timeout(Duration::from_secs(90))  // アイドル接続のタイムアウト

        // タイムアウト設定
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(120))  // 推論は長時間かかる可能性

        // Keep-Alive 設定
        .tcp_keepalive(Duration::from_secs(30))

        // その他の最適化
        .tcp_nodelay(true)
        .http2_prior_knowledge()  // HTTP/2 を優先

        .build()
        .expect("Failed to create HTTP client")
}
```

## 2. 待機機構の改善

### 決定

**Tokio の非同期プリミティブ**（Notify + timeout）を使用

### 理由

- 外部依存なし
- Tokio エコシステムとの統合が容易
- メモリ効率が高い

### 代替案比較

| 方式 | 説明 | メリット | デメリット |
|------|------|---------|-----------|
| Tokio Notify | 非同期通知 | シンプル、軽量 | ブロードキャスト非対応 |
| tokio::sync::broadcast | ブロードキャスト | 複数待機者対応 | メモリ消費 |
| crossbeam-channel | ロックフリーキュー | 高スループット | 同期的 |
| Redis Pub/Sub | 分散キュー | スケーラブル | 外部依存 |

### 実装方法

```rust
// router/src/balancer/wait_queue.rs

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time::timeout;

/// 待機キュー
pub struct WaitQueue {
    notify: Notify,
    waiting_count: AtomicUsize,
    max_waiting: usize,
}

impl WaitQueue {
    pub fn new(max_waiting: usize) -> Self {
        Self {
            notify: Notify::new(),
            waiting_count: AtomicUsize::new(0),
            max_waiting,
        }
    }

    /// 待機キューに参加
    pub async fn wait(&self, timeout_duration: Duration) -> Result<(), WaitError> {
        let current = self.waiting_count.fetch_add(1, Ordering::SeqCst);
        let load_ratio = current as f64 / self.max_waiting as f64;

        // 段階的バックプレッシャー
        if load_ratio >= 0.8 {
            self.waiting_count.fetch_sub(1, Ordering::SeqCst);
            return Err(WaitError::QueueFull);
        }

        // 50-80% の場合は遅延付き受付
        if load_ratio >= 0.5 {
            let delay = Duration::from_millis((load_ratio * 100.0) as u64);
            tokio::time::sleep(delay).await;
        }

        // タイムアウト付き待機
        match timeout(timeout_duration, self.notify.notified()).await {
            Ok(()) => {
                self.waiting_count.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            }
            Err(_) => {
                self.waiting_count.fetch_sub(1, Ordering::SeqCst);
                Err(WaitError::Timeout)
            }
        }
    }

    /// ノードが利用可能になったことを通知
    pub fn notify_one(&self) {
        self.notify.notify_one();
    }

    /// 現在の待機数を取得
    pub fn waiting_count(&self) -> usize {
        self.waiting_count.load(Ordering::SeqCst)
    }
}

pub enum WaitError {
    QueueFull,
    Timeout,
}
```

## 3. ノード選択の最適化

### 決定

**mini-moka (非同期キャッシュ)** を使用

### 理由

- Rust 製の高性能キャッシュ
- TTL とサイズ制限をサポート
- 非同期 API を提供

### 代替案比較

| ライブラリ | 説明 | メリット | デメリット |
|-----------|------|---------|-----------|
| mini-moka | 非同期キャッシュ | 軽量、高性能 | 機能限定 |
| moka | フル機能キャッシュ | 高機能 | サイズ大 |
| dashmap | 並行ハッシュマップ | シンプル | TTL なし |
| カスタム LRU | 独自実装 | 完全制御 | 実装コスト |

### 実装方法

```rust
// router/src/balancer/node_cache.rs

use mini_moka::sync::Cache;
use std::time::Duration;

/// ノード選択キャッシュ
pub struct NodeSelectionCache {
    cache: Cache<String, String>,  // model_id -> runtime_id
}

impl NodeSelectionCache {
    pub fn new() -> Self {
        Self {
            cache: Cache::builder()
                .max_capacity(1000)
                .time_to_live(Duration::from_secs(5))  // 5秒 TTL
                .build(),
        }
    }

    /// キャッシュからノードを取得
    pub fn get(&self, model_id: &str) -> Option<String> {
        self.cache.get(model_id)
    }

    /// キャッシュにノードを保存
    pub fn insert(&self, model_id: String, runtime_id: String) {
        self.cache.insert(model_id, runtime_id);
    }

    /// 特定モデルのキャッシュを無効化
    pub fn invalidate(&self, model_id: &str) {
        self.cache.invalidate(model_id);
    }

    /// 全キャッシュを無効化
    pub fn invalidate_all(&self) {
        self.cache.invalidate_all();
    }
}
```

## 4. バックプレッシャー戦略

### 決定

**3段階制御**を採用

### 理由

- シンプルで予測可能
- 段階的な劣化により雪崩効果を防止
- 設定変更が容易

### 段階設計

| 負荷率 | 状態 | アクション | HTTPレスポンス |
|--------|------|----------|---------------|
| 0-50% | 正常 | 即座に受付 | - |
| 50-80% | 警告 | 遅延付き受付 | 200 (遅延あり) |
| 80-100% | 過負荷 | 即座に拒否 | 503 Service Unavailable |

### 実装方法

```rust
// router/src/balancer/backpressure.rs

/// バックプレッシャー状態
#[derive(Debug, Clone, Copy)]
pub enum BackpressureState {
    Normal,      // 0-50%
    Warning,     // 50-80%
    Overloaded,  // 80-100%
}

/// バックプレッシャーコントローラー
pub struct BackpressureController {
    max_queue_size: usize,
    warning_threshold: f64,   // 0.5
    overload_threshold: f64,  // 0.8
}

impl BackpressureController {
    pub fn new(max_queue_size: usize) -> Self {
        Self {
            max_queue_size,
            warning_threshold: 0.5,
            overload_threshold: 0.8,
        }
    }

    /// 現在の状態を評価
    pub fn evaluate(&self, current_queue_size: usize) -> BackpressureState {
        let load = current_queue_size as f64 / self.max_queue_size as f64;

        if load >= self.overload_threshold {
            BackpressureState::Overloaded
        } else if load >= self.warning_threshold {
            BackpressureState::Warning
        } else {
            BackpressureState::Normal
        }
    }

    /// 遅延時間を計算（警告状態用）
    pub fn calculate_delay(&self, current_queue_size: usize) -> Duration {
        let load = current_queue_size as f64 / self.max_queue_size as f64;

        if load < self.warning_threshold {
            Duration::ZERO
        } else {
            // 50% → 0ms, 80% → 100ms の線形補間
            let delay_ms = ((load - self.warning_threshold)
                / (self.overload_threshold - self.warning_threshold)
                * 100.0) as u64;
            Duration::from_millis(delay_ms)
        }
    }
}
```

## 5. メトリクス設計

### 決定

**prometheus クレート**を使用

### メトリクス一覧

```rust
// router/src/metrics/load.rs

use prometheus::{Counter, Gauge, Histogram, register_counter, register_gauge, register_histogram};

lazy_static! {
    // リクエスト処理時間
    pub static ref REQUEST_DURATION: Histogram = register_histogram!(
        "llm_router_request_duration_seconds",
        "Request processing duration",
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]
    ).unwrap();

    // 待機キューサイズ
    pub static ref QUEUE_SIZE: Gauge = register_gauge!(
        "llm_router_queue_size",
        "Current wait queue size"
    ).unwrap();

    // バックプレッシャー拒否数
    pub static ref BACKPRESSURE_REJECTIONS: Counter = register_counter!(
        "llm_router_backpressure_rejections_total",
        "Total requests rejected by backpressure"
    ).unwrap();

    // 接続プール使用率
    pub static ref POOL_CONNECTIONS: Gauge = register_gauge!(
        "llm_router_pool_connections",
        "Active connections in pool"
    ).unwrap();

    // キャッシュヒット率
    pub static ref CACHE_HITS: Counter = register_counter!(
        "llm_router_cache_hits_total",
        "Node selection cache hits"
    ).unwrap();

    pub static ref CACHE_MISSES: Counter = register_counter!(
        "llm_router_cache_misses_total",
        "Node selection cache misses"
    ).unwrap();
}
```

## 参考リソース

- [reqwest Connection Pooling](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html)
- [Tokio Sync Primitives](https://docs.rs/tokio/latest/tokio/sync/)
- [mini-moka Cache](https://docs.rs/mini-moka/latest/mini_moka/)
- [Prometheus Rust](https://docs.rs/prometheus/latest/prometheus/)
- [Backpressure Patterns](https://mechanical-sympathy.blogspot.com/2012/05/apply-back-pressure-when-overloaded.html)
