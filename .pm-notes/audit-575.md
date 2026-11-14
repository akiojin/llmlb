# audit-575 結果要約 (OpenAI互換APIゲートウェイ)
- 残: US-024 のみ (a) delete_endpoint_model_handler が削除成功後に endpoint_models 即時 resync/cache invalidate しない (endpoints.rs:1276, 5分周期まで残留=AS-024-1不成立) (b) AS-024-1〜4 の contract テスト無し
- 実装済み未チェック: US-006 Anthropic Messages API 一式 (+#614 tool_use), US-029 canonical 統一, US-024本体, SPEC Delta 各種 → チェック/PENDING 更新
- 要判断: US-024 の vllm/openai_compatible reject と xllm 等 unsupported の区別 (現実装は一律400)
- 補足: spec artifact 二重化 (最新=comment 4601798618, 旧=4182651288) → 整理推奨
- 推奨: 子Issue 1件 + チェック更新。子Issue 完了後に #575 close 可
