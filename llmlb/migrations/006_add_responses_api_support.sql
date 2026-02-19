-- SPEC-0f1de549: Open Responses API対応
-- エンドポイントにResponses API対応フラグを追加

-- endpoints テーブルに supports_responses_api 列を追加
ALTER TABLE endpoints ADD COLUMN supports_responses_api INTEGER NOT NULL DEFAULT 0;

-- endpoint_models テーブルに supported_apis 列を追加（JSON配列）
ALTER TABLE endpoint_models ADD COLUMN supported_apis TEXT DEFAULT '["chat_completions"]';
