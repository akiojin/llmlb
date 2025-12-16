# データモデル: 管理ダッシュボード

**機能ID**: `SPEC-712c20cf` | **日付**: 2025-10-31

## 概要

管理ダッシュボード機能で使用するデータモデル定義。既存の`Node`型を再利用し、新規に`SystemStats`型を追加する。

## エンティティ

### 1. Node (既存)

**説明**: ノード情報を表す構造体（既存の`common/src/types.rs`で定義済み）

> 2025-11-01 追記: `loaded_models: Vec<String>` を追加し、ノードがLLM runtimeにロード済みのモデル一覧を保持する。ダッシュボードの「モデル」列および詳細モーダルで参照する。

**フィールド**:
```rust
pub struct Node {
    pub id: Uuid,
    pub machine_name: String,
    pub ip_address: String,
    pub runtime_version: String,
    pub status: NodeStatus,
    pub registered_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub online_since: Option<DateTime<Utc>>,
    pub system_info: SystemInfo,
}

pub enum NodeStatus {
    Online,
    Offline,
}

pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub cpu_cores: u32,
    pub total_memory: u64,
}
```

**検証ルール**:
- `machine_name`: 空文字列禁止
- `ip_address`: 有効なIPv4/IPv6アドレス
- `runtime_version`: セマンティックバージョニング形式

**ダッシュボードでの使用**:
- ノード一覧表示
- オンライン/オフラインステータス表示
- 稼働時間計算（直近でオンラインになった時刻=`online_since` と現在時刻の差分、未設定時は0秒）

### 2. SystemStats (新規)

**説明**: システム全体の統計情報

**フィールド**:
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemStats {
    pub total_nodes: usize,
    pub online_nodes: usize,
    pub offline_nodes: usize,
    pub total_requests: u64,      // 将来拡張
    pub avg_response_time_ms: u32, // 将来拡張
    pub errors_count: u64,         // 将来拡張
}
```

**検証ルール**:
- `total_nodes >= 0`
- `online_nodes + offline_nodes == total_nodes`
- `total_requests >= 0`
- `avg_response_time_ms >= 0`

**計算方法**:
- `total_nodes`: NodeRegistryの全ノード数
- `online_nodes`: `status == NodeStatus::Online`の数
- `offline_nodes`: `status == NodeStatus::Offline`の数
- `total_requests`, `avg_response_time_ms`, `errors_count`: 将来拡張（初期実装では0）

### 3. NodeWithUptime (新規レスポンス型)

**説明**: ダッシュボードAPI用のノード情報（稼働時間を含む）

**フィールド**:
```rust
#[derive(Debug, Serialize)]
pub struct NodeWithUptime {
    pub id: Uuid,
    pub machine_name: String,
    pub ip_address: String,
    pub status: NodeStatus,
    pub runtime_version: String,
    pub registered_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub uptime_seconds: i64,
}
```

**計算方法**:
- `uptime_seconds`: `online_since` があれば `status` に応じて `now` または `last_seen` との差分（秒）。`online_since` が未設定なら 0。

**API変換**:
```rust
impl From<Node> for NodeWithUptime {
    fn from(node: Node) -> Self {
        let now = Utc::now();
        let uptime_seconds = if let Some(online_since) = node.online_since {
            let end = if matches!(node.status, NodeStatus::Online) {
                now
            } else {
                node.last_seen
            };
            (end - online_since).num_seconds().max(0)
        } else {
            0
        };
        Self {
            id: node.id,
            machine_name: node.machine_name,
            ip_address: node.ip_address,
            status: node.status,
            runtime_version: node.runtime_version,
            registered_at: node.registered_at,
            last_seen: node.last_seen,
            uptime_seconds,
        }
    }
}
```

### 4. NodeMetrics (将来拡張、SPEC-589f2df1依存)

**説明**: ノードのパフォーマンスメトリクス（将来拡張用）

**フィールド**:
```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub node_id: Uuid,
    pub cpu_usage: f64,           // %
    pub memory_usage: f64,        // %
    pub active_requests: u32,     // 件
    pub avg_response_time_ms: u32,// ms
    pub timestamp: DateTime<Utc>,
}
```

**注**: SPEC-589f2df1（ロードバランシングシステム）でメトリクス収集機能が実装された後に使用可能。

## エンティティ関係図

```
┌─────────────────┐
│     Node       │ (既存)
│─────────────────│
│ + id            │
│ + machine_name  │
│ + ip_address    │
│ + status        │
│ + ...           │
└─────────────────┘
         │
         │ 1:1 変換
         ▼
┌──────────────────┐
│ NodeWithUptime  │ (新規レスポンス型)
│──────────────────│
│ + id             │
│ + machine_name   │
│ + uptime_seconds │
│ + ...            │
└──────────────────┘

┌──────────────────┐
│   SystemStats    │ (新規)
│──────────────────│
│ + total_nodes    │
│ + online_nodes   │
│ + ...            │
└──────────────────┘

┌──────────────────┐
│  NodeMetrics    │ (将来拡張)
│──────────────────│
│ + node_id        │
│ + cpu_usage      │
│ + ...            │
└──────────────────┘
         │
         │ 1:N
         │
         ▼
┌─────────────────┐
│     Node        │
└─────────────────┘
```

## 状態遷移

### NodeStatus

```
    register
┌──────────────┐
│   (未登録)    │
└──────────────┘
       │
       │ POST /v0/nodes
       ▼
┌──────────────┐
│    Online    │ ◄──────┐
└──────────────┘        │
       │                │ POST /v0/health (X-Node-Token)
       │ timeout        │
       ▼                │
┌──────────────┐        │
│   Offline    │ ───────┘
└──────────────┘
```

## データフロー

### ノード一覧取得
```
Client ─GET /v0/dashboard/nodes→ Router
                                        │
                                        │ NodeRegistry.list_all()
                                        ▼
                                    Vec<Node>
                                        │
                                        │ map(Node → DashboardNode)
                                        ▼
                                  Vec<DashboardNode>
                                        │
                                        │ JSON
                                        ▼
Client ◄──────────────────────────── Response
```

### システム統計取得
```
Client ─GET /v0/dashboard/stats→ Router
                                       │
                                       │ NodeRegistry.list_all()
                                       ▼
                                   Vec<Node>
                                       │
                                       │ count(), filter()
                                       ▼
                                   SystemStats
                                       │
                                       │ JSON
                                       ▼
Client ◄─────────────────────────── Response
```

## ファイル配置

```
common/src/
├── types.rs              # Node, NodeStatus, SystemInfo (既存)
└── dashboard.rs          # NodeWithUptime, SystemStats (新規)

router/src/
├── api/
│   └── dashboard.rs      # ダッシュボードAPI実装
└── registry/
    └── mod.rs            # NodeRegistry (既存)
```

## テストデータ

### サンプルNode
```json
{
  "id": "123e4567-e89b-12d3-a456-426614174000",
  "machine_name": "server-01",
  "ip_address": "192.168.1.100",
  "status": "Online",
  "runtime_version": "0.1.0",
  "registered_at": "2025-10-31T10:00:00Z",
  "last_seen": "2025-10-31T12:30:00Z",
  "system_info": {
    "os": "Linux",
    "arch": "x86_64",
    "cpu_cores": 8,
    "total_memory": 16777216
  }
}
```

### サンプルSystemStats
```json
{
  "total_nodes": 10,
  "online_nodes": 8,
  "offline_nodes": 2,
  "total_requests": 0,
  "successful_requests": 0,
  "failed_requests": 0,
  "total_active_requests": 0,
  "average_response_time_ms": null
}
```

## 将来拡張

### メトリクス可視化（SPEC-589f2df1実装後）
- `NodeMetrics`の実装
- メトリクス収集API (`POST /v0/health` / `X-Node-Token`)
- メトリクス取得API (`GET /v0/dashboard/metrics/:node_id`)
- リクエスト履歴グラフ用のデータ構造

### リクエスト履歴
- `RequestHistory`構造体
- 時系列データ（1分単位のリクエスト数）
- リングバッファによるメモリ管理（最新1時間分のみ保持）

---
*このデータモデルは plan.md Phase 1 の成果物です*
