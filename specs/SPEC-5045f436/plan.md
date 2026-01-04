# SPEC-5045f436: 実装計画

## 技術スタック

| カテゴリ | 技術 |
|----------|------|
| 言語 | Rust |
| データベース | SQLite（既存） |
| トークナイザ | tiktoken-rs |
| API | axum（既存） |
| フロントエンド | 既存SPAダッシュボード |

## アーキテクチャ概要

```text
[APIリクエスト]
      ↓
[ノード処理] → [レスポンス(usage含む)]
      ↓
[LoadManager.finish_request()]
      ↓
[トークン抽出] ← usageフィールド or tiktoken推定
      ↓
[NodeLoadState累積] + [SQLite永続化]
      ↓
[Dashboard API] → [統計表示]
```

## 修正対象ファイル

### Phase 1: データモデル

| ファイル | 変更内容 |
|----------|----------|
| `router/migrations/003_add_token_statistics.sql` | 新規マイグレーション作成 |
| `common/src/protocol.rs` | RequestResponseRecordにトークンフィールド追加 |

### Phase 2: コア実装

| ファイル | 変更内容 |
|----------|----------|
| `router/Cargo.toml` | tiktoken-rs依存追加 |
| `router/src/balancer/mod.rs` | NodeLoadStateにトークンフィールド追加 |
| `router/src/balancer/mod.rs` | finish_request()でトークン集計 |
| `router/src/db/request_history.rs` | insert_record()でトークン値保存 |

### Phase 3: API拡張

| ファイル | 変更内容 |
|----------|----------|
| `router/src/api/dashboard.rs` | DashboardNode/Statsにトークンフィールド追加 |
| `router/src/api/dashboard.rs` | 新規統計エンドポイント追加 |

### Phase 4: ダッシュボードUI

| ファイル | 変更内容 |
|----------|----------|
| `router/src/dashboard/` | ノード一覧にトークン統計表示 |
| `router/src/dashboard/` | 専用統計ページ追加 |

## データモデル設計

### SQLiteマイグレーション

```sql
-- router/migrations/003_add_token_statistics.sql

-- request_history テーブルにトークンカラム追加
ALTER TABLE request_history ADD COLUMN input_tokens INTEGER;
ALTER TABLE request_history ADD COLUMN output_tokens INTEGER;
ALTER TABLE request_history ADD COLUMN total_tokens INTEGER;

-- 集計用インデックス
CREATE INDEX idx_request_history_tokens ON request_history(timestamp DESC, model);
CREATE INDEX idx_request_history_node_tokens ON request_history(node_id, timestamp DESC);
```

### Rust構造体

```rust
// common/src/protocol.rs - RequestResponseRecord拡張
pub struct RequestResponseRecord {
    // ... 既存フィールド ...
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

// router/src/balancer/mod.rs - NodeLoadState拡張
struct NodeLoadState {
    // ... 既存フィールド ...
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_tokens: u64,
}

// router/src/api/dashboard.rs - DashboardNode拡張
pub struct DashboardNode {
    // ... 既存フィールド ...
    pub total_input_tokens: Option<u64>,
    pub total_output_tokens: Option<u64>,
    pub average_tokens_per_request: Option<f32>,
}

// router/src/api/dashboard.rs - DashboardStats拡張
pub struct DashboardStats {
    // ... 既存フィールド ...
    pub total_input_tokens: Option<u64>,
    pub total_output_tokens: Option<u64>,
}
```

## API設計

### 既存エンドポイント拡張

**GET /api/dashboard/nodes**

レスポンスにトークン統計フィールドを追加：

```json
{
  "nodes": [
    {
      "id": "...",
      "total_requests": 100,
      "total_input_tokens": 50000,
      "total_output_tokens": 30000,
      "average_tokens_per_request": 800.0
    }
  ]
}
```

**GET /api/dashboard/stats**

レスポンスにトークン統計フィールドを追加：

```json
{
  "total_requests": 1000,
  "total_input_tokens": 500000,
  "total_output_tokens": 300000
}
```

### 新規エンドポイント

**GET /api/dashboard/stats/tokens**

全体トークン統計：

```json
{
  "total_input_tokens": 500000,
  "total_output_tokens": 300000,
  "total_tokens": 800000,
  "by_node": [...],
  "by_model": [...]
}
```

**GET /api/dashboard/stats/tokens/daily**

日次トークン統計（クエリパラメータ: from, to）

**GET /api/dashboard/stats/tokens/monthly**

月次トークン統計（クエリパラメータ: from, to）

## トークン抽出ロジック

### 優先順位

1. **usageフィールド抽出**（優先）
   - OpenAI互換レスポンスからusageオブジェクトを抽出
   - prompt_tokens, completion_tokens, total_tokensを取得

2. **tiktoken推定**（フォールバック）
   - usageフィールドがない場合に使用
   - リクエスト/レスポンスのテキストからトークン数を推定

### ストリーミング対応

- SSEチャンクごとにトークン情報を累積
- 最終チャンクまたはdoneイベントで集計完了

### エラー応答対応

- エラー応答でも可能な範囲でトークンをカウント
- 入力トークンは常にカウント可能（リクエスト側で把握）

## 依存関係

### 新規依存

```toml
# router/Cargo.toml
[dependencies]
tiktoken-rs = "0.5"  # OpenAI互換トークナイザ
```

## テスト計画

### ユニットテスト

- トークン抽出ロジック
- tiktoken推定ロジック
- NodeLoadState累積ロジック

### 統合テスト

- finish_request()でのトークン記録
- SQLite永続化・読み出し
- Dashboard API応答

### E2Eテスト

- 完全なリクエスト→統計表示フロー
- ストリーミングリクエストのトークン累積

## リスクと対策

| リスク | 対策 |
|--------|------|
| tiktoken推定精度 | usageフィールド優先、推定値はフラグ付き |
| DB書き込み負荷 | バッチ更新、非同期書き込み |
| ストリーミング複雑性 | チャンク累積ロジックの十分なテスト |

## 憲章チェック

- [x] TDD: テストファースト実装
- [x] シンプルさ: 既存パターン踏襲、最小限の変更
- [x] LLM最適化: 該当なし（内部機能）

## 変更履歴

| 日付 | 変更内容 |
|------|----------|
| 2026-01-04 | 初版作成 |
