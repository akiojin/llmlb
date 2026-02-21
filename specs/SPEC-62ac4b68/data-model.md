# データモデル: IPアドレスロギング＆クライアント分析

**機能ID**: `SPEC-62ac4b68` | **日付**: 2026-02-20

## スキーマ変更

### マイグレーション 017: client_ip_tracking

#### 既存テーブル変更: request_history

| カラム | 型 | 変更 | 説明 |
|--------|------|------|------|
| client_ip | TEXT | 既存（値をNULLから実IPに変更） | クライアントIPアドレス |
| api_key_id | TEXT | **新規追加** | APIキーUUID（api_keysテーブル参照） |

#### 新規インデックス

| インデックス名 | 対象 | 用途 |
|---------------|------|------|
| idx_request_history_client_ip | client_ip | IPフィルター・集計 |
| idx_request_history_api_key_id | api_key_id | APIキー別集計 |

#### 新規テーブル: settings

| カラム | 型 | 制約 | 説明 |
|--------|------|------|------|
| key | TEXT | PRIMARY KEY | 設定キー |
| value | TEXT | NOT NULL | 設定値（JSON文字列） |
| updated_at | TEXT | NOT NULL | 最終更新日時（ISO8601） |

初期データ: `key='ip_alert_threshold', value='100'`

## Rust構造体変更

### RequestResponseRecord (protocol.rs)

```text
既存フィールド:
  id: Uuid
  timestamp: DateTime<Utc>
  request_type: RequestType
  model: String
  node_id: Uuid
  node_machine_name: String
  node_ip: IpAddr
  client_ip: Option<IpAddr>        ← 既存（値をNoneから実IPに変更）
  request_body: Value
  response_body: Option<Value>
  duration_ms: u64
  status: RecordStatus
  completed_at: DateTime<Utc>
  input_tokens: Option<u32>
  output_tokens: Option<u32>
  total_tokens: Option<u32>

追加フィールド:
  api_key_id: Option<Uuid>          ← 新規追加
```

### RecordFilter (request_history.rs)

```text
既存フィールド:
  model: Option<String>
  node_id: Option<Uuid>
  status: Option<FilterStatus>
  start_time: Option<DateTime<Utc>>
  end_time: Option<DateTime<Utc>>

追加フィールド:
  client_ip: Option<String>         ← 新規追加
```

### RequestHistoryRow (request_history.rs)

```text
追加フィールド:
  api_key_id: Option<String>        ← 新規追加
```

## 新規API レスポンス型

### ClientIpRanking

```text
ip: String                          # IPアドレス（IPv6は/64プレフィックス）
request_count: u64                  # リクエスト数
last_seen: DateTime<Utc>            # 最終アクセス時刻
is_alert: bool                      # 閾値超過フラグ
api_key_count: u32                  # 使用APIキー数
```

### UniqueIpTimelinePoint

```text
hour: String                        # 時間帯（ISO8601）
unique_ips: u32                     # ユニークIP数
```

### ModelDistribution

```text
model: String                       # モデル名
request_count: u64                  # リクエスト数
percentage: f64                     # 割合（%）
```

### HeatmapCell

```text
day_of_week: u8                     # 曜日（0=月, 6=日）
hour: u8                            # 時間帯（0-23）
count: u64                          # リクエスト数
```

### ClientDetail

```text
ip: String                          # IPアドレス
total_requests: u64                 # 合計リクエスト数
first_seen: DateTime<Utc>           # 初回アクセス
last_seen: DateTime<Utc>            # 最終アクセス
recent_requests: Vec<RequestResponseRecord>  # 直近リクエスト
model_distribution: Vec<ModelDistribution>   # モデル分布
hourly_activity: Vec<HourlyActivity>         # 時間帯パターン
api_keys: Vec<ClientApiKeyUsage>             # 使用APIキー
```

## エンティティ関係

```text
request_history
  ├── client_ip → (集計単位: IPv4個別 / IPv6は/64グループ)
  ├── api_key_id → api_keys.id (LEFT JOIN、NULLable)
  └── node_id → endpoints.id

settings
  └── key='ip_alert_threshold' → Clientsタブの異常検知閾値
```
