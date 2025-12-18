# タスク: SPEC-fba3c39d ONNX LLM推論（実装）

## 方針
- 既存のスタブ実装を「テスト専用」に閉じ込め、本番経路は実推論へ置換する。
- chat_template と tokenizer は **車輪の再発明を避け**、既存OSSの採用を優先する。
- 既存の Playwright / Rust / C++ テストがすべてパスすること。

## Tasks
- [x] T001 `scripts/export_hf_to_onnx.py` を `text-generation-with-past` に更新
- [x] T002 `llm-node` に tokenizer + chat_template レンダリング実装を追加
- [x] T003 `InferenceEngine::generateChat()` を実推論へ置換（kv-cache 生成ループ）
- [x] T004 `InferenceEngine::generateChatStream()` を実ストリーミングへ置換
- [x] T005 `node/src/api/openai_endpoints.cpp` の SSE を token-by-token に接続
- [x] T006 Playwright のテストモデルを LLM 向けへ更新（mnist → text-generation）
- [x] T007 `make quality-checks` / `ctest` / Playwright を実行し全合格
