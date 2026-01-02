# データモデル: ロードバランシングシステム

## エンティティ定義

### ノードメトリクス

```rust
// router/src/balancer/metrics.rs

use std::time::Instant;

/// ノードのパフォーマンスメトリクス
#[derive(Debug, Clone)]
pub struct NodeMetrics {
    /// ノードID
    pub node_id: String,

    /// GPU使用率（0-100%）
    pub gpu_usage: f64,

    /// VRAM使用率（0-100%）
    pub vram_usage: f64,

    /// GPU温度（℃）
    pub gpu_temperature: Option<f64>,

    /// 処理中のリクエスト数
    pub active_requests: u32,

    /// 平均レスポンスタイム（ms）
    pub avg_response_time_ms: Option<f64>,

    /// CPU使用率（参考値）
    pub cpu_usage: Option<f64>,

    /// メモリ使用率（参考値）
    pub memory_usage: Option<f64>,

    /// 最終更新時刻
    pub updated_at: Instant,
}

impl Default for NodeMetrics {
    fn default() -> Self {
        Self {
            node_id: String::new(),
            gpu_usage: 0.0,
            vram_usage: 0.0,
            gpu_temperature: None,
            active_requests: 0,
            avg_response_time_ms: None,
            cpu_usage: None,
            memory_usage: None,
            updated_at: Instant::now(),
        }
    }
}
```

### GPU能力スコア

```rust
// router/src/balancer/gpu_score.rs

/// GPUの能力スコア（0-10000）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GpuCapabilityScore(pub u32);

impl GpuCapabilityScore {
    /// VRAMサイズからスコアを計算
    pub fn from_vram_gb(vram_gb: u32) -> Self {
        // 基本スコア: VRAM(GB) × 100
        Self(vram_gb * 100)
    }

    /// スコア値を取得
    pub fn score(&self) -> u32 {
        self.0
    }
}

/// ノードのGPU情報
#[derive(Debug, Clone)]
pub struct GpuInfo {
    /// GPUモデル名
    pub model: String,

    /// VRAM容量（GB）
    pub vram_gb: u32,

    /// 能力スコア
    pub capability_score: GpuCapabilityScore,
}
```

### ロードバランサー

```rust
// router/src/balancer/mod.rs

use std::sync::Arc;
use dashmap::DashMap;
use std::collections::VecDeque;

/// ロードバランシング方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalancerMode {
    /// ラウンドロビン（デフォルト）
    RoundRobin,

    /// メトリクスベース
    Metrics,
}

impl Default for LoadBalancerMode {
    fn default() -> Self {
        Self::RoundRobin
    }
}

/// ロードバランサー
pub struct LoadBalancer {
    /// 現在のモード
    mode: LoadBalancerMode,

    /// ノードメトリクス（node_id -> metrics）
    metrics: Arc<DashMap<String, NodeMetrics>>,

    /// メトリクス履歴（node_id -> 履歴）
    history: Arc<DashMap<String, VecDeque<MetricsPoint>>>,

    /// ラウンドロビンのインデックス
    round_robin_index: std::sync::atomic::AtomicUsize,
}

/// メトリクス履歴ポイント
#[derive(Debug, Clone)]
pub struct MetricsPoint {
    /// GPU使用率
    pub gpu_usage: f64,

    /// VRAM使用率
    pub vram_usage: f64,

    /// アクティブリクエスト数
    pub active_requests: u32,

    /// 記録時刻
    pub timestamp: std::time::Instant,
}
```

### ノード選択結果

```rust
// router/src/balancer/selection.rs

/// ノード選択結果
#[derive(Debug, Clone)]
pub enum NodeSelectionResult {
    /// ノードが選択された
    Selected {
        node_id: String,
        reason: SelectionReason,
    },

    /// 利用可能なノードがない
    NoAvailableNodes,

    /// すべてのノードが高負荷
    AllNodesOverloaded {
        fallback_node_id: String,
    },
}

/// 選択理由
#[derive(Debug, Clone)]
pub enum SelectionReason {
    /// GPU負荷が最小
    LowestGpuLoad { gpu_usage: f64 },

    /// VRAM空きが最大
    HighestVramAvailable { vram_usage: f64 },

    /// アクティブリクエストが最少
    LeastActiveRequests { count: u32 },

    /// GPU能力スコアが最高
    HighestCapability { score: u32 },

    /// ラウンドロビンで選択
    RoundRobin { index: usize },
}
```

### 負荷判定

```rust
// router/src/balancer/load_check.rs

/// 負荷状態
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    /// アイドル（GPU <= 80%, VRAM <= 90%）
    Idle,

    /// 高負荷（GPU > 80% または VRAM > 90%）
    Busy,

    /// メトリクス不明
    Unknown,
}

/// 負荷判定設定
#[derive(Debug, Clone)]
pub struct LoadThresholds {
    /// GPU使用率の閾値（デフォルト: 80%）
    pub gpu_usage_threshold: f64,

    /// VRAM使用率の閾値（デフォルト: 90%）
    pub vram_usage_threshold: f64,

    /// アクティブリクエスト数の閾値（デフォルト: 10）
    pub active_requests_threshold: u32,
}

impl Default for LoadThresholds {
    fn default() -> Self {
        Self {
            gpu_usage_threshold: 80.0,
            vram_usage_threshold: 90.0,
            active_requests_threshold: 10,
        }
    }
}
```

## 検証ルール

| フィールド | ルール | エラーメッセージ |
|-----------|--------|------------------|
| `gpu_usage` | 0.0 - 100.0 | "GPU usage must be between 0 and 100" |
| `vram_usage` | 0.0 - 100.0 | "VRAM usage must be between 0 and 100" |
| `gpu_temperature` | 0.0 - 150.0 | "GPU temperature must be between 0 and 150" |
| `active_requests` | >= 0 | "Active requests cannot be negative" |
| `capability_score` | 0 - 10000 | "Capability score must be between 0 and 10000" |
| `gpu_usage_threshold` | 0.0 - 100.0 | "GPU threshold must be between 0 and 100" |
| `vram_usage_threshold` | 0.0 - 100.0 | "VRAM threshold must be between 0 and 100" |

## 関係図

```text
┌─────────────────────────────────────────────────────────────────┐
│                        LoadBalancer                              │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                     ノード選択フロー                          ││
│  │                                                              ││
│  │  [Request] ──→ get_available_nodes()                         ││
│  │                       │                                      ││
│  │                       ▼                                      ││
│  │              オンラインノードをフィルタ                       ││
│  │                       │                                      ││
│  │           ┌───────────┼───────────┐                         ││
│  │           ▼           ▼           ▼                         ││
│  │      Metrics Mode  RoundRobin  Fallback                     ││
│  │           │                                                  ││
│  │           ▼                                                  ││
│  │    GPU使用率 <= 80% フィルタ                                 ││
│  │           │                                                  ││
│  │           ▼                                                  ││
│  │    VRAM使用率 <= 90% フィルタ                                ││
│  │           │                                                  ││
│  │           ▼                                                  ││
│  │    GPU能力スコア順ソート                                     ││
│  │           │                                                  ││
│  │           ▼                                                  ││
│  │    最少アクティブリクエストで選択                            ││
│  │           │                                                  ││
│  │           ▼                                                  ││
│  │  [NodeSelectionResult]                                       ││
│  │                                                              ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ NodeMetrics  │  │ GpuInfo      │  │ LoadState    │          │
│  │ - gpu_usage  │  │ - model      │  │ - Idle       │          │
│  │ - vram_usage │  │ - vram_gb    │  │ - Busy       │          │
│  │ - active_req │  │ - score      │  │ - Unknown    │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
└─────────────────────────────────────────────────────────────────┘
```

## 環境変数

```bash
# ロードバランサーモード
LOAD_BALANCER_MODE=metrics  # metrics | round_robin

# 負荷閾値
LLM_ROUTER_GPU_THRESHOLD=80      # GPU使用率閾値（%）
LLM_ROUTER_VRAM_THRESHOLD=90     # VRAM使用率閾値（%）
LLM_ROUTER_ACTIVE_REQ_THRESHOLD=10  # アクティブリクエスト閾値
```

## メトリクス形式

```text
# 選択統計
llm_router_node_selections_total{node_id="node-1",reason="lowest_gpu"} 1500
llm_router_node_selections_total{node_id="node-2",reason="round_robin"} 500

# ノード負荷
llm_router_node_gpu_usage{node_id="node-1"} 45.5
llm_router_node_vram_usage{node_id="node-1"} 78.2
llm_router_node_active_requests{node_id="node-1"} 3

# 選択時間
llm_router_node_selection_duration_seconds_bucket{le="0.001"} 9500
llm_router_node_selection_duration_seconds_bucket{le="0.01"} 10000
llm_router_node_selection_duration_seconds_sum 5.2
llm_router_node_selection_duration_seconds_count 10000
```
