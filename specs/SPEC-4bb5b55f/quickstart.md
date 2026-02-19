# クイックスタート: エンドポイント×モデル単位TPS可視化

**機能ID**: `SPEC-4bb5b55f` | **日付**: 2026-02-19

## 変更対象ファイル一覧

### バックエンド（Rust）

| ファイル | 変更内容 |
|----------|---------|
| `llmlb/migrations/016_add_tps_columns.sql` | **新規**: endpoint_daily_statsにカラム追加 |
| `llmlb/src/balancer/mod.rs` | **改修**: TpsTracker追加、TPS EMA計算 |
| `llmlb/src/db/endpoint_daily_stats.rs` | **改修**: upsert_daily_statsにトークン・時間追加 |
| `llmlb/src/api/proxy.rs` | **改修**: record_endpoint_request_statsにトークン・時間引数追加 |
| `llmlb/src/api/openai.rs` | **改修**: 全complete地点でトークン・時間をstats関数に渡す |
| `llmlb/src/api/dashboard.rs` | **改修**: model-tps APIハンドラー追加 |
| `llmlb/src/api/mod.rs` | **改修**: model-tpsルート追加 |
| `llmlb/src/events/mod.rs` | **改修**: TpsUpdatedイベント追加 |
| `llmlb/src/types/endpoint.rs` | **改修**: EndpointType判定ヘルパー追加 |

### フロントエンド（TypeScript/React）

| ファイル | 変更内容 |
|----------|---------|
| `llmlb/src/web/dashboard/src/pages/Dashboard.tsx` | **改修**: モデルTPSテーブル追加 |

## ビルド手順

```bash
# 1. マイグレーション適用（cargo test時に自動実行）
cargo test

# 2. ダッシュボードビルド
pnpm --filter @llm/dashboard build

# 3. 全体ビルド
cargo build

# 4. 品質チェック
make quality-checks
```

## 動作確認

```bash
# TPS APIの確認
curl -H "x-api-key: sk_debug" \
  http://localhost:8080/api/endpoints/{endpoint_id}/model-tps

# ダッシュボード確認
open http://localhost:8080/dashboard
```
