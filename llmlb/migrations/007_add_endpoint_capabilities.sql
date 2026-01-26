-- SPEC-66555000: Add capabilities column to endpoints table
-- This allows endpoints to declare their supported features (image_generation, audio_transcription, etc.)

ALTER TABLE endpoints ADD COLUMN capabilities TEXT DEFAULT '["chat_completion"]';
