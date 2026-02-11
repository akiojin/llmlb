-- SPEC-76643000: エンドポイント単位リクエスト統計
-- endpointsテーブルにカウンタカラム追加 + 日次集計テーブル新規作成

-- endpointsテーブルにリクエストカウンタを追加（永続的、クリーンアップ対象外）
ALTER TABLE endpoints ADD COLUMN total_requests INTEGER NOT NULL DEFAULT 0;
ALTER TABLE endpoints ADD COLUMN successful_requests INTEGER NOT NULL DEFAULT 0;
ALTER TABLE endpoints ADD COLUMN failed_requests INTEGER NOT NULL DEFAULT 0;

-- エンドポイント×モデル×日付粒度の日次集計テーブル
-- FK制約なし: エンドポイント削除時も集計データを保持（孤児データ許容）
CREATE TABLE IF NOT EXISTS endpoint_daily_stats (
    endpoint_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    date TEXT NOT NULL,
    total_requests INTEGER NOT NULL DEFAULT 0,
    successful_requests INTEGER NOT NULL DEFAULT 0,
    failed_requests INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (endpoint_id, model_id, date)
);

-- 日次チャート用: エンドポイント+日付での検索を高速化
CREATE INDEX IF NOT EXISTS idx_daily_stats_endpoint_date
    ON endpoint_daily_stats (endpoint_id, date);

-- 日次バッチ用: 日付での全件検索を高速化
CREATE INDEX IF NOT EXISTS idx_daily_stats_date
    ON endpoint_daily_stats (date);
