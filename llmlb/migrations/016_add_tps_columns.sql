-- SPEC-4bb5b55f: エンドポイント×モデル単位TPS可視化
-- endpoint_daily_statsテーブルにTPS計算用カラムを追加
ALTER TABLE endpoint_daily_stats
  ADD COLUMN total_output_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE endpoint_daily_stats
  ADD COLUMN total_duration_ms INTEGER NOT NULL DEFAULT 0;
