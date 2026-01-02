# データモデル: ヘルスチェックシステム

## エンティティ定義

### ノード状態

```rust
// router/src/registry/node.rs

use std::time::Instant;
use serde::{Deserialize, Serialize};

/// ノードの状態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeState {
    /// 承認待ち（登録直後）
    Pending,

    /// 登録中（承認済み、初期化中）
    Registering,

    /// オンライン（正常稼働）
    Online,

    /// オフライン（応答なし）
    Offline,
}

impl Default for NodeState {
    fn default() -> Self {
        Self::Pending
    }
}

impl NodeState {
    /// リクエスト振り分け対象かどうか
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Online)
    }
}
```

### ハートビートリクエスト

```rust
// router/src/api/health.rs

use serde::{Deserialize, Serialize};

/// ハートビートリクエスト
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeartbeatRequest {
    /// ノードID
    pub node_id: String,

    /// GPUメトリクス
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_metrics: Option<GpuMetrics>,

    /// システムメトリクス（参考値）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_metrics: Option<SystemMetrics>,

    /// 処理中リクエスト数
    pub active_requests: u32,

    /// ロード済みモデル一覧
    #[serde(default)]
    pub loaded_models: Vec<String>,
}

/// GPUメトリクス
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GpuMetrics {
    /// GPU使用率（0-100%）
    pub usage: f64,

    /// VRAM使用率（0-100%）
    pub vram_usage: f64,

    /// GPU温度（℃）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// VRAM使用量（MB）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vram_used_mb: Option<u64>,

    /// VRAM総容量（MB）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vram_total_mb: Option<u64>,
}

/// システムメトリクス（参考値）
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SystemMetrics {
    /// CPU使用率（0-100%）
    pub cpu_usage: f64,

    /// メモリ使用率（0-100%）
    pub memory_usage: f64,
}
```

### ハートビートレスポンス

```rust
// router/src/api/health.rs

/// ハートビートレスポンス
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeartbeatResponse {
    /// 処理結果
    pub status: HeartbeatStatus,

    /// サーバー時刻（Unix timestamp）
    pub server_time: u64,
}

/// ハートビート処理結果
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeartbeatStatus {
    /// 正常受信
    Ok,

    /// ノードが未登録
    NotRegistered,

    /// トークン無効
    InvalidToken,
}
```

### ヘルスモニター

```rust
// router/src/health/monitor.rs

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// ヘルスモニター設定
#[derive(Debug, Clone)]
pub struct HealthMonitorConfig {
    /// 監視間隔
    pub check_interval: Duration,

    /// ノードタイムアウト
    pub node_timeout: Duration,
}

impl Default for HealthMonitorConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(10),
            node_timeout: Duration::from_secs(60),
        }
    }
}

/// ヘルスモニター
pub struct HealthMonitor {
    /// 設定
    config: HealthMonitorConfig,

    /// ノードレジストリへの参照
    registry: Arc<RwLock<NodeRegistry>>,

    /// 最終チェック時刻
    last_check: Arc<RwLock<Instant>>,
}

/// ノードのヘルス情報
#[derive(Debug, Clone)]
pub struct NodeHealthInfo {
    /// ノードID
    pub node_id: String,

    /// 現在の状態
    pub state: NodeState,

    /// 最終ハートビート受信時刻
    pub last_seen: Instant,

    /// タイムアウトまでの残り時間
    pub time_until_timeout: Duration,

    /// 連続ハートビート成功回数
    pub consecutive_heartbeats: u32,
}
```

### 状態遷移

```rust
// router/src/registry/transition.rs

/// 状態遷移イベント
#[derive(Debug, Clone)]
pub enum StateTransition {
    /// ノード登録
    Registered {
        node_id: String,
    },

    /// 管理者承認
    Approved {
        node_id: String,
        approved_by: String,
    },

    /// 初期化完了（オンラインへ）
    Ready {
        node_id: String,
    },

    /// ハートビート受信（オフライン→オンライン）
    Recovered {
        node_id: String,
        downtime: Duration,
    },

    /// タイムアウト（オンライン→オフライン）
    TimedOut {
        node_id: String,
        last_seen: Instant,
    },

    /// 手動オフライン
    ManualOffline {
        node_id: String,
        reason: String,
    },
}
```

### タイムアウト情報

```rust
// router/src/health/timeout.rs

use std::time::{Duration, Instant};

/// タイムアウト判定
#[derive(Debug, Clone)]
pub struct TimeoutCheck {
    /// 最終ハートビート時刻
    pub last_seen: Instant,

    /// 現在時刻
    pub now: Instant,

    /// タイムアウト期間
    pub timeout: Duration,

    /// 経過時間
    pub elapsed: Duration,

    /// タイムアウトしているか
    pub is_timed_out: bool,
}

impl TimeoutCheck {
    pub fn new(last_seen: Instant, timeout: Duration) -> Self {
        let now = Instant::now();
        let elapsed = now.duration_since(last_seen);
        let is_timed_out = elapsed > timeout;

        Self {
            last_seen,
            now,
            timeout,
            elapsed,
            is_timed_out,
        }
    }
}
```

## 検証ルール

| フィールド | ルール | エラーメッセージ |
|-----------|--------|------------------|
| `node_id` | 非空文字列 | "Node ID is required" |
| `gpu_metrics.usage` | 0.0 - 100.0 | "GPU usage must be between 0 and 100" |
| `gpu_metrics.vram_usage` | 0.0 - 100.0 | "VRAM usage must be between 0 and 100" |
| `active_requests` | >= 0 | "Active requests cannot be negative" |
| `check_interval` | 1秒 - 60秒 | "Check interval must be between 1 and 60 seconds" |
| `node_timeout` | 10秒 - 300秒 | "Node timeout must be between 10 and 300 seconds" |

## 関係図

```text
┌─────────────────────────────────────────────────────────────────┐
│                       Health Monitor                             │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                   ハートビートフロー                          ││
│  │                                                              ││
│  │  [Node]                                                      ││
│  │    │                                                         ││
│  │    │ POST /v0/health                                         ││
│  │    │ X-Node-Token: xxx                                       ││
│  │    ▼                                                         ││
│  │  ┌────────────────┐                                         ││
│  │  │ Token Validate │                                         ││
│  │  └───────┬────────┘                                         ││
│  │          │                                                   ││
│  │          ▼                                                   ││
│  │  ┌────────────────┐                                         ││
│  │  │ Update Metrics │ → NodeMetrics                           ││
│  │  └───────┬────────┘                                         ││
│  │          │                                                   ││
│  │          ▼                                                   ││
│  │  ┌────────────────┐                                         ││
│  │  │ Update last_seen│ → Instant::now()                       ││
│  │  └───────┬────────┘                                         ││
│  │          │                                                   ││
│  │          ▼                                                   ││
│  │  ┌────────────────┐                                         ││
│  │  │ Check & Recover│ Offline → Online                        ││
│  │  └───────┬────────┘                                         ││
│  │          │                                                   ││
│  │          ▼                                                   ││
│  │  [HeartbeatResponse]                                         ││
│  │                                                              ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                   バックグラウンド監視                        ││
│  │                                                              ││
│  │  [Timer: 10秒間隔]                                           ││
│  │          │                                                   ││
│  │          ▼                                                   ││
│  │  ┌────────────────┐                                         ││
│  │  │ Check All Nodes│                                         ││
│  │  └───────┬────────┘                                         ││
│  │          │                                                   ││
│  │          ▼                                                   ││
│  │    ┌─────┴─────┐                                            ││
│  │    ▼           ▼                                            ││
│  │  [OK]     [Timeout]                                         ││
│  │             │                                                ││
│  │             ▼                                                ││
│  │    ┌────────────────┐                                       ││
│  │    │ Mark Offline   │ Online → Offline                      ││
│  │    └────────────────┘                                       ││
│  │                                                              ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ NodeState    │  │ last_seen    │  │ timeout      │          │
│  │ - Pending    │  │ (Instant)    │  │ (Duration)   │          │
│  │ - Online     │  │              │  │ default: 60s │          │
│  │ - Offline    │  │              │  │              │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└─────────────────────────────────────────────────────────────────┘
```

## 環境変数

```bash
# ノード側設定
LLM_NODE_HEARTBEAT_SECS=30        # ハートビート送信間隔（秒）

# ルーター側設定
LLM_ROUTER_HEALTH_CHECK_INTERVAL=10  # 監視間隔（秒）
LLM_ROUTER_NODE_TIMEOUT=60           # タイムアウト（秒）

# レガシー環境変数（後方互換）
HEALTH_CHECK_INTERVAL=10             # → LLM_ROUTER_HEALTH_CHECK_INTERVAL
NODE_TIMEOUT=60                      # → LLM_ROUTER_NODE_TIMEOUT
```

## メトリクス形式

```text
# ハートビート
llm_router_heartbeats_received_total{node_id="node-1"} 1000
llm_router_heartbeats_received_total{node_id="node-2"} 995

# ノード状態
llm_router_node_state{node_id="node-1",state="online"} 1
llm_router_node_state{node_id="node-2",state="online"} 1
llm_router_node_state{node_id="node-3",state="offline"} 1

# 状態遷移
llm_router_node_transitions_total{from="online",to="offline"} 5
llm_router_node_transitions_total{from="offline",to="online"} 5

# タイムアウト
llm_router_node_timeouts_total{node_id="node-3"} 3
```
