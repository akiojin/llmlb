# タスク: xLLM Responses API対応

**機能ID**: `SPEC-1a74455c`
**日付**: 2026-01-26
**入力**: spec.md / plan.md / data-model.md

## フォーマット

- `[ ]` 未着手 / `[x]` 完了
- `[P]` 並列実行可能
- 変更対象ファイルを明記する

## Phase 1: テストファースト (TDD RED)

- [x] T001 [P] `xllm/tests/contract/openai_api_test.cpp`
  - /v1/responses 基本応答テスト
  - /v1/responses ストリーミングテスト
  - /v1/responses 入力必須テスト
  - /v1/responses 配列入力テスト

- [x] T002 [P] `xllm/tests/integration/openai_endpoints_test.cpp`
  - /v1/responses 統合テスト（object/usage確認）

- [x] T003 [P] `xllm/tests/integration/node_endpoints_test.cpp`
  - /health に supports_responses_api が含まれることを確認

## Phase 2: コア実装

- [x] T004 `xllm/src/api/openai_endpoints.cpp`
  - Responses API入力パース（文字列・配列）
  - instructions対応（systemメッセージ）
  - max_output_tokens対応

- [x] T005 `xllm/src/api/openai_endpoints.cpp`
  - `POST /v1/responses` 追加
  - レスポンス生成（response object, output, usage）
  - ストリーミングSSE対応

- [x] T006 `xllm/src/api/node_endpoints.cpp`
  - /health に supports_responses_api 追加

## Phase 3: 仕上げ

- [x] T007 テスト実行（contract / integration）
- [x] T008 spec.md のステータス更新

## 依存関係

```text
T001-T003 → T004-T006 → T007-T008
```
