# データモデル: ルーター負荷最適化

## エンティティ定義

### 待機キュー

```rust
// router/src/balancer/wait_queue.rs

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

/// 待機キュー設定
#[derive(Debug, Clone)]
pub struct WaitQueueConfig {
    /// 最大待機数
    pub max_waiting: usize,

    /// デフォルトタイムアウト
    pub default_timeout: Duration,

    /// 警告閾値（0.0-1.0）
    pub warning_threshold: f64,

    /// 過負荷閾値（0.0-1.0）
    pub overload_threshold: f64,
}

impl Default for WaitQueueConfig {
    fn default() -> Self {
        Self {
            max_waiting: 100,
            default_timeout: Duration::from_secs(30),
            warning_threshold: 0.5,
            overload_threshold: 0.8,
        }
    }
}

/// 待機キュー
pub struct WaitQueue {
    /// 設定
    config: WaitQueueConfig,

    /// 通知機構
    notify: Notify,

    /// 現在の待機数
    waiting_count: AtomicUsize,
}

/// 待機エントリ
#[derive(Debug)]
pub struct WaitEntry {
    /// 待機開始時刻
    pub started_at: Instant,

    /// タイムアウト時刻
    pub timeout_at: Instant,

    /// リクエストID
    pub request_id: String,

    /// 要求モデル
    pub model_id: String,
}

/// 待機結果
#[derive(Debug)]
pub enum WaitResult {
    /// ノードが利用可能になった
    Ready { runtime_id: String },

    /// タイムアウト
    Timeout { waited: Duration },

    /// キュー満杯で拒否
    Rejected { queue_size: usize },
}

/// 待機エラー
#[derive(Debug, Clone)]
pub enum WaitError {
    /// キュー満杯
    QueueFull {
        current_size: usize,
        max_size: usize,
    },

    /// タイムアウト
    Timeout {
        waited: Duration,
        timeout: Duration,
    },

    /// キャンセル
    Cancelled,
}
```

### 接続プール

```rust
// router/src/proxy/pool.rs

use std::time::Duration;

/// 接続プール設定
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// ホストあたり最大アイドル接続数
    pub max_idle_per_host: usize,

    /// アイドル接続タイムアウト
    pub idle_timeout: Duration,

    /// 接続タイムアウト
    pub connect_timeout: Duration,

    /// リクエストタイムアウト
    pub request_timeout: Duration,

    /// TCP Keep-Alive 間隔
    pub tcp_keepalive: Duration,

    /// TCP_NODELAY 有効化
    pub tcp_nodelay: bool,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_idle_per_host: 32,
            idle_timeout: Duration::from_secs(90),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(120),
            tcp_keepalive: Duration::from_secs(30),
            tcp_nodelay: true,
        }
    }
}

/// 接続プール統計
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    /// アクティブ接続数
    pub active_connections: usize,

    /// アイドル接続数
    pub idle_connections: usize,

    /// 接続作成数（累計）
    pub connections_created: u64,

    /// 接続再利用数（累計）
    pub connections_reused: u64,
}
```

### ノード選択キャッシュ

```rust
// router/src/balancer/node_cache.rs

use std::time::{Duration, Instant};

/// キャッシュ設定
#[derive(Debug, Clone)]
pub struct NodeCacheConfig {
    /// 最大エントリ数
    pub max_capacity: u64,

    /// TTL（Time To Live）
    pub ttl: Duration,
}

impl Default for NodeCacheConfig {
    fn default() -> Self {
        Self {
            max_capacity: 1000,
            ttl: Duration::from_secs(5),
        }
    }
}

/// キャッシュエントリ
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// ノードID
    pub runtime_id: String,

    /// 作成時刻
    pub created_at: Instant,

    /// 有効期限
    pub expires_at: Instant,
}

/// キャッシュ統計
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// ヒット数
    pub hits: u64,

    /// ミス数
    pub misses: u64,

    /// 現在のエントリ数
    pub size: u64,

    /// 無効化数
    pub invalidations: u64,
}
```

### バックプレッシャー

```rust
// router/src/balancer/backpressure.rs

use std::time::Duration;

/// バックプレッシャー状態
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureState {
    /// 正常（0-50%）
    Normal,

    /// 警告（50-80%）- 遅延付き受付
    Warning,

    /// 過負荷（80-100%）- 拒否
    Overloaded,
}

/// バックプレッシャー設定
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// 最大キューサイズ
    pub max_queue_size: usize,

    /// 警告閾値
    pub warning_threshold: f64,

    /// 過負荷閾値
    pub overload_threshold: f64,

    /// 最大遅延
    pub max_delay: Duration,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 100,
            warning_threshold: 0.5,
            overload_threshold: 0.8,
            max_delay: Duration::from_millis(100),
        }
    }
}

/// バックプレッシャー決定
#[derive(Debug, Clone)]
pub struct BackpressureDecision {
    /// 現在の状態
    pub state: BackpressureState,

    /// 追加遅延
    pub delay: Duration,

    /// 受付可否
    pub accept: bool,

    /// 現在の負荷率
    pub load_ratio: f64,
}
```

### ロードマネージャー統合

```rust
// router/src/balancer/load_manager.rs

use std::sync::Arc;

/// 最適化されたロードマネージャー
pub struct OptimizedLoadManager {
    /// 待機キュー
    pub wait_queue: Arc<WaitQueue>,

    /// ノード選択キャッシュ
    pub node_cache: Arc<NodeSelectionCache>,

    /// バックプレッシャーコントローラー
    pub backpressure: BackpressureController,

    /// 設定
    pub config: LoadManagerConfig,
}

/// ロードマネージャー設定
#[derive(Debug, Clone)]
pub struct LoadManagerConfig {
    pub wait_queue: WaitQueueConfig,
    pub connection_pool: ConnectionPoolConfig,
    pub node_cache: NodeCacheConfig,
    pub backpressure: BackpressureConfig,
}

impl Default for LoadManagerConfig {
    fn default() -> Self {
        Self {
            wait_queue: WaitQueueConfig::default(),
            connection_pool: ConnectionPoolConfig::default(),
            node_cache: NodeCacheConfig::default(),
            backpressure: BackpressureConfig::default(),
        }
    }
}
```

## 検証ルール

| フィールド | ルール | エラーメッセージ |
|-----------|--------|--------------------|
| `max_waiting` | 1以上1000以下 | "Max waiting must be between 1 and 1000" |
| `default_timeout` | 1秒以上300秒以下 | "Timeout must be between 1 and 300 seconds" |
| `warning_threshold` | 0.0以上1.0未満 | "Warning threshold must be between 0.0 and 1.0" |
| `overload_threshold` | warning より大きく1.0以下 | "Overload threshold must be greater than warning" |
| `max_idle_per_host` | 1以上100以下 | "Max idle per host must be between 1 and 100" |
| `ttl` | 100ms以上60秒以下 | "TTL must be between 100ms and 60s" |

## 関係図

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                      OptimizedLoadManager                                │
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │                    リクエスト受付フロー                           │   │
│  │                                                                   │   │
│  │  [Request] ──→ [BackpressureController] ──→ 状態判定              │   │
│  │                         │                                         │   │
│  │           ┌─────────────┼─────────────┐                          │   │
│  │           ▼             ▼             ▼                          │   │
│  │       Normal        Warning      Overloaded                      │   │
│  │       (即座に)    (遅延付き)     (拒否)                          │   │
│  │           │             │             │                          │   │
│  │           └──────┬──────┘             │                          │   │
│  │                  ▼                    ▼                          │   │
│  │           [WaitQueue]           503 Error                        │   │
│  │                  │                                               │   │
│  │                  ▼                                               │   │
│  │           [NodeCache] ─── hit ──→ [Node]                         │   │
│  │                │                                                 │   │
│  │              miss                                                │   │
│  │                │                                                 │   │
│  │                ▼                                                 │   │
│  │          [NodeSelector] ──→ [Node] ──→ [Cache Update]            │   │
│  │                                                                   │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │ WaitQueue   │  │ NodeCache   │  │ Backpressure│  │ ConnPool    │    │
│  │ - notify    │  │ - cache     │  │ - thresholds│  │ - idle      │    │
│  │ - count     │  │ - ttl       │  │ - delay     │  │ - keepalive │    │
│  │ - timeout   │  │ - capacity  │  │ - state     │  │ - nodelay   │    │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘    │
└─────────────────────────────────────────────────────────────────────────┘
```

## 設定ファイル形式

### config.toml

```toml
[load_manager]
# 待機キュー設定
max_waiting = 100
default_timeout_secs = 30

# バックプレッシャー設定
warning_threshold = 0.5
overload_threshold = 0.8

[connection_pool]
max_idle_per_host = 32
idle_timeout_secs = 90
connect_timeout_secs = 5
request_timeout_secs = 120

[node_cache]
max_capacity = 1000
ttl_secs = 5
```

### 環境変数

```bash
# 待機キュー
LLMLB_MAX_WAITING=100
LLMLB_WAIT_TIMEOUT_SECS=30

# バックプレッシャー
LLMLB_WARNING_THRESHOLD=0.5
LLMLB_OVERLOAD_THRESHOLD=0.8

# 接続プール
LLMLB_POOL_MAX_IDLE=32
LLMLB_POOL_IDLE_TIMEOUT_SECS=90

# ノードキャッシュ
LLMLB_CACHE_MAX_CAPACITY=1000
LLMLB_CACHE_TTL_SECS=5
```

## メトリクス形式

```text
# 待機キューサイズ
llm_router_queue_size 45

# リクエスト処理時間
llm_router_request_duration_seconds_bucket{le="0.01"} 1000
llm_router_request_duration_seconds_bucket{le="0.05"} 2500
llm_router_request_duration_seconds_bucket{le="0.1"} 4000
llm_router_request_duration_seconds_sum 250.5
llm_router_request_duration_seconds_count 5000

# バックプレッシャー拒否
llm_router_backpressure_rejections_total 50

# キャッシュ効率
llm_router_cache_hits_total 9000
llm_router_cache_misses_total 1000

# 接続プール
llm_router_pool_connections 24
```
