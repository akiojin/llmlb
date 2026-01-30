# 実装計画: xLLM Responses API対応

**機能ID**: `SPEC-1a74455c`
**作成日**: 2026-01-26

## 方針

- xLLMのOpenAI互換エンドポイントに`/v1/responses`を追加する
- 入力は文字列またはメッセージ配列を受け付ける
- 既存のChat Completions実装を再利用し、レスポンス形式をResponses APIに合わせる
- /healthのレスポンスにResponses API対応フラグを追加する

## 対象コンポーネント

- `xllm/src/api/openai_endpoints.cpp`
- `xllm/src/api/node_endpoints.cpp`
- `xllm/tests/contract/openai_api_test.cpp`
- `xllm/tests/integration/openai_endpoints_test.cpp`
- `xllm/tests/integration/node_endpoints_test.cpp`

## フェーズ

### Phase 1: テスト準備 (TDD RED)

- Contractテスト: /v1/responsesの基本応答、ストリーミング、入力必須
- Integrationテスト: /v1/responsesのレスポンスとusage確認
- /healthにsupports_responses_apiが含まれることのテスト

### Phase 2: 実装

- 入力パーサー追加（文字列・配列）
- `POST /v1/responses`のハンドラ追加
- usage計算（入力/出力トークン概算）
- ストリーミングSSEのイベント整形
- /healthにsupports_responses_apiを追加

### Phase 3: 仕上げ

- テスト実行（contract / integration）
- 既存APIへの影響確認

## 受け入れ基準

- /v1/responsesが基本リクエストとstream=trueで動作する
- 400エラーが正しく返る
- /healthでsupports_responses_apiが取得できる
- 追加テストがパスする
