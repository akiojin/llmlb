-- SPEC-f8e3a1b7: llmlb アーキテクチャ刷新
-- エンドポイントにデバイス情報・推論レイテンシカラムを追加

-- endpoints テーブルに device_info 列を追加
-- xLLMの/api/systemから取得したGPU/CPU情報をJSON形式で保存
ALTER TABLE endpoints ADD COLUMN device_info TEXT;

-- endpoints テーブルに inference_latency_ms 列を追加
-- 推論リクエストの平均レイテンシ（EMA α=0.2で計算）
ALTER TABLE endpoints ADD COLUMN inference_latency_ms REAL;
