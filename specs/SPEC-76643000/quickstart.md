# クイックスタート: エンドポイント単位リクエスト統計

**機能ID**: `SPEC-76643000` | **日付**: 2026-02-10

## 実装順序

### ステップ1: マイグレーション＋型定義

1. `llmlb/migrations/014_add_endpoint_request_stats.sql` を作成
2. `llmlb/src/types/endpoint.rs` にカウンタフィールドを追加
3. EndpointDailyStats構造体を定義

### ステップ2: DB操作レイヤー

1. `llmlb/src/db/endpoint_daily_stats.rs` を新規作成
2. `llmlb/src/db/endpoints.rs` にカウンタ更新関数を追加
3. `llmlb/src/db/mod.rs` にモジュール登録

### ステップ3: リクエスト処理フローへの組み込み

1. `balancer/mod.rs` の `finish_request()`/`finish_request_with_tokens()` に
   カウンタ更新呼び出しを追加
2. tokio::spawnで非同期実行（レイテンシ影響なし）

### ステップ4: 日次バッチタスク

1. `main.rs` に日次バッチタスクの起動を追加
2. start_cleanup_taskと同じパターン

### ステップ5: Dashboard APIの拡張

1. DashboardEndpoint構造体にカウンタフィールド追加
2. collect_endpoints()でカウンタ値を取得
3. 新規APIエンドポイント3つを追加

### ステップ6: フロントエンド一覧テーブル

1. api.tsの型拡張
2. EndpointTable.tsxにRequestsカラム追加

### ステップ7: フロントエンド詳細モーダル

1. 数値カード4枚の追加
2. EndpointRequestChart.tsx（Recharts積み上げ棒グラフ）新規作成
3. モデル別テーブル追加

## ローカル検証コマンド

```bash
# フォーマットチェック
cargo fmt --check > /dev/null 2>&1 && echo "OK" || echo "FAIL"

# Clippy
cargo clippy -- -D warnings 2>&1 | tail -20

# テスト
cargo test 2>&1 | grep -E "(test result|FAILED|passed|failed)" | tail -10

# 全体品質チェック
make quality-checks 2>&1 | tail -50
```

## 検証シナリオ

1. サーバー起動後、エンドポイントを1つ登録
2. OpenAI互換APIでリクエストを数件送信
3. ダッシュボードでRequestsカラムに数値が表示されることを確認
4. エンドポイント詳細モーダルで統計カードとチャートを確認
5. 7日以上経過してもカウンタが減らないことを確認
