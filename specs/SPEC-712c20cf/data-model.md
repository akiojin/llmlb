# データモデル: 管理ダッシュボード

**SPEC-ID**: SPEC-712c20cf
**日付**: 2025-10-31

## 概要

ダッシュボード機能では、既存のエージェント管理モデルを再利用し、統計情報の集約のための新規モデルのみ追加します。

## 既存モデル（再利用）

### Agent

エージェント情報を表すモデル（`coordinator/src/registry/mod.rs` に既存）

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// エージェントの一意識別子（UUID）
    pub id: String,

    /// ホスト名
    pub hostname: String,

    /// IPアドレス
    pub ip_address: String,

    /// Ollamaのバージョン
    pub ollama_version: String,

    /// エージェントの現在ステータス
    pub status: AgentStatus,

    /// 最後のハートビート受信時刻
    pub last_heartbeat: DateTime<Utc>,

    /// エージェント登録時刻
    pub registered_at: DateTime<Utc>,
}
```

**検証ルール**:
- `id`: 空文字列不可、UUID形式
- `hostname`: 空文字列不可
- `ip_address`: 空文字列不可、IPv4またはIPv6形式
- `ollama_version`: 空文字列不可

### AgentStatus

エージェントの稼働状態を表す列挙型（`coordinator/src/registry/mod.rs` に既存）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    /// オンライン（ハートビート受信中）
    Online,

    /// オフライン（タイムアウト）
    Offline,
}
```

**状態遷移**:
```
┌─────────┐
│ Online  │ ←──── 新規登録時
└────┬────┘
     │
     │ ハートビートタイムアウト（60秒）
     ↓
┌─────────┐
│ Offline │
└────┬────┘
     │
     │ ハートビート受信
     ↓
┌─────────┐
│ Online  │
└─────────┘
```

## 新規モデル

### DashboardStats

システム全体の統計情報を表すモデル

**ファイル**: `coordinator/src/dashboard/stats.rs`（新規作成）

```rust
use serde::{Deserialize, Serialize};

/// ダッシュボード統計情報
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DashboardStats {
    /// 登録エージェント総数
    pub total_agents: usize,

    /// オンラインエージェント数
    pub online_agents: usize,

    /// オフラインエージェント数
    pub offline_agents: usize,

    /// 総リクエスト処理数（Phase 3で実装、現在はNone）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_requests: Option<u64>,

    /// 平均レスポンスタイム（ミリ秒、Phase 3で実装、現在はNone）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_response_time_ms: Option<f64>,

    /// エラー総数（Phase 3で実装、現在はNone）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_count: Option<u64>,
}

impl DashboardStats {
    /// エージェントリストから統計情報を生成
    pub fn from_agents(agents: &[Agent]) -> Self {
        let total_agents = agents.len();
        let online_agents = agents
            .iter()
            .filter(|a| a.status == AgentStatus::Online)
            .count();
        let offline_agents = total_agents - online_agents;

        Self {
            total_agents,
            online_agents,
            offline_agents,
            total_requests: None,      // Phase 3で実装
            avg_response_time_ms: None, // Phase 3で実装
            error_count: None,          // Phase 3で実装
        }
    }
}
```

**検証ルール**:
- `total_agents = online_agents + offline_agents` （不変条件）
- `online_agents >= 0`
- `offline_agents >= 0`
- `total_requests >= 0` （Someの場合）
- `avg_response_time_ms >= 0.0` （Someの場合）
- `error_count >= 0` （Someの場合）

**JSON シリアライゼーション例**:
```json
{
  "total_agents": 5,
  "online_agents": 3,
  "offline_agents": 2
}
```

Phase 3実装後:
```json
{
  "total_agents": 5,
  "online_agents": 3,
  "offline_agents": 2,
  "total_requests": 1523,
  "avg_response_time_ms": 45.2,
  "error_count": 3
}
```

### AgentMetrics（Phase 3で実装）

エージェントごとのパフォーマンスメトリクス（将来実装）

```rust
use serde::{Deserialize, Serialize};

/// エージェントメトリクス（Phase 3で実装）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    /// エージェントID
    pub agent_id: String,

    /// CPU使用率（パーセント、0.0-100.0）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_usage: Option<f64>,

    /// メモリ使用率（パーセント、0.0-100.0）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_usage: Option<f64>,

    /// 処理中のリクエスト数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_requests: Option<u32>,

    /// 総リクエスト処理数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_requests: Option<u64>,

    /// 平均レスポンスタイム（ミリ秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_response_time_ms: Option<f64>,
}
```

**検証ルール**:
- `cpu_usage`: 0.0 <= x <= 100.0
- `memory_usage`: 0.0 <= x <= 100.0
- `active_requests >= 0`
- `total_requests >= 0`
- `avg_response_time_ms >= 0.0`

## エンティティ関係図

```
┌──────────────┐
│ Agent        │
├──────────────┤
│ id           │
│ hostname     │
│ ip_address   │
│ status       │◄──┐
│ ...          │   │
└──────────────┘   │
                   │
                   │
┌──────────────────┴──────┐
│ AgentStatus (enum)      │
├─────────────────────────┤
│ • Online                │
│ • Offline               │
└─────────────────────────┘

┌──────────────────────────┐
│ DashboardStats           │
├──────────────────────────┤
│ total_agents             │
│ online_agents            │
│ offline_agents           │
│ total_requests (opt)     │
│ avg_response_time (opt)  │
│ error_count (opt)        │
└──────────────────────────┘
         ▲
         │ from_agents()
         │
┌────────┴────────┐
│ Vec<Agent>      │
└─────────────────┘
```

## データフロー

1. **エージェント一覧取得**:
   ```
   AgentRegistry → Vec<Agent> → JSON → Frontend
   ```

2. **統計情報取得**:
   ```
   AgentRegistry → Vec<Agent> → DashboardStats::from_agents() → JSON → Frontend
   ```

3. **リアルタイム更新**:
   ```
   Frontend (5秒ポーリング) → GET /api/dashboard/agents
                             → GET /api/dashboard/stats
                             → Chart.js更新
   ```

## テスト戦略

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_stats_from_agents() {
        let agents = vec![
            Agent { status: AgentStatus::Online, ... },
            Agent { status: AgentStatus::Online, ... },
            Agent { status: AgentStatus::Offline, ... },
        ];

        let stats = DashboardStats::from_agents(&agents);

        assert_eq!(stats.total_agents, 3);
        assert_eq!(stats.online_agents, 2);
        assert_eq!(stats.offline_agents, 1);
        assert!(stats.total_requests.is_none());
    }

    #[test]
    fn test_dashboard_stats_invariant() {
        let stats = DashboardStats {
            total_agents: 10,
            online_agents: 7,
            offline_agents: 3,
            ...
        };

        assert_eq!(stats.total_agents, stats.online_agents + stats.offline_agents);
    }
}
```

## まとめ

- **既存モデル**: `Agent`, `AgentStatus` を再利用
- **新規モデル**: `DashboardStats` のみ（Phase 1）
- **将来拡張**: `AgentMetrics`（Phase 3）
- **設計原則**: シンプル、不変条件維持、型安全
