# タスク: OpenAI互換API完全準拠

**機能ID**: `SPEC-24157000` | **日付**: 2026-01-05
**入力**: `/specs/SPEC-24157000/`の設計ドキュメント
**前提条件**: plan.md, research.md, data-model.md

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- 説明には正確なファイルパスを含める

## Phase 3.1: セットアップ

- [x] T001 `node/include/api/openai_endpoints.h` に TokenUsage, LogprobInfo 構造体を宣言
- [x] T002 `node/include/core/engine_types.h` に presence_penalty, frequency_penalty, n フィールドを追加

## Phase 3.2: テストファースト (TDD) - 3.3の前に完了必須

**重要: これらのテストは記述され、実装前に失敗する必要がある**

### P1: usage計算・ID生成テスト

- [x] T003 [P] `node/tests/contract/openai_api_test.cpp` に ChatCompletionsReturnsUsage テスト追加
- [x] T004 [P] `node/tests/contract/openai_api_test.cpp` に CompletionsReturnsUsage テスト追加
- [x] T005 [P] `node/tests/contract/openai_api_test.cpp` に ResponseIdIsUnique テスト追加
- [x] T006 [P] `node/tests/contract/openai_api_test.cpp` に CreatedTimestampIsValid テスト追加

### P2: penalty・logprobsテスト

- [x] T007 [P] `node/tests/contract/openai_api_test.cpp` に PresencePenaltyAccepted テスト追加
- [x] T008 [P] `node/tests/contract/openai_api_test.cpp` に FrequencyPenaltyAccepted テスト追加
- [x] T009 [P] `node/tests/contract/openai_api_test.cpp` に PenaltyOutOfRangeReturns400 テスト追加
- [x] T010 [P] `node/tests/contract/openai_api_test.cpp` に LogprobsReturnsRealValues テスト追加
- [x] T011 [P] `node/tests/contract/openai_api_test.cpp` に TopLogprobsReturnsNItems テスト追加

### P3: nパラメータテスト

- [x] T012 [P] `node/tests/contract/openai_api_test.cpp` に NParameterReturnsMultipleChoices テスト追加
- [x] T013 [P] `node/tests/contract/openai_api_test.cpp` に NParameterOutOfRangeReturns400 テスト追加

### 統合テスト

- [x] T014 `node/tests/integration/openai_endpoints_test.cpp` に UsageMatchesActualTokenCount テスト追加
- [x] T015 `node/tests/integration/openai_endpoints_test.cpp` に LogprobsMatchesModelOutput テスト追加

## Phase 3.3: コア実装 (テストが失敗した後のみ)

### ユーティリティ関数

- [x] T016 `node/src/api/openai_endpoints.cpp` に count_tokens() 関数実装（概算方式）
- [x] T017 `node/src/api/openai_endpoints.cpp` に generate_response_id() 関数実装（タイムスタンプ+乱数）
- [x] T018 `node/src/api/openai_endpoints.cpp` に get_current_timestamp() 関数実装

### P1: usage計算・ID生成実装

- [x] T019 `node/src/api/openai_endpoints.cpp` の chatCompletions() でusageフィールドを計算・追加
- [x] T020 `node/src/api/openai_endpoints.cpp` の completions() でusageフィールドを計算・追加
- [x] T021 `node/src/api/openai_endpoints.cpp` の chatCompletions() でidフィールドを動的生成
- [x] T022 `node/src/api/openai_endpoints.cpp` の completions() でidフィールドを動的生成
- [x] T023 `node/src/api/openai_endpoints.cpp` の chatCompletions() でcreatedを現在時刻に設定
- [x] T024 `node/src/api/openai_endpoints.cpp` の completions() でcreatedを現在時刻に設定

### P2: penaltyパラメータ実装

- [x] T025 `node/src/api/openai_endpoints.cpp` の parseInferenceParams() で presence_penalty パース追加
- [x] T026 `node/src/api/openai_endpoints.cpp` の parseInferenceParams() で frequency_penalty パース追加
- [x] T027 `node/src/api/openai_endpoints.cpp` の validateSamplingParams() で penalty 範囲チェック追加
- [x] T028 `node/src/core/llama_engine.cpp` で presence_penalty を llama_sampler に適用
- [x] T029 `node/src/core/llama_engine.cpp` で frequency_penalty を llama_sampler に適用

### P2: logprobs実装

- [x] T030 `node/src/api/openai_endpoints.cpp` に compute_pseudo_logprob() 関数実装（疑似値、将来llama_get_logits統合予定）
- [x] T031 `node/src/api/openai_endpoints.cpp` に build_logprobs() 関数改善（負の実数を返す）
- [x] T032 `node/src/api/openai_endpoints.cpp` の chatCompletions() で logprobs=true 時に実値を返す
- [x] T033 `node/src/api/openai_endpoints.cpp` の completions() で logprobs=true 時に実値を返す
- [x] T034 `node/src/api/openai_endpoints.cpp` で top_logprobs パラメータ対応

### P3: nパラメータ実装

- [x] T035 `node/src/api/openai_endpoints.cpp` の parseInferenceParams() で n パース追加
- [x] T036 `node/src/api/openai_endpoints.cpp` の validateSamplingParams() で n 範囲チェック追加（1-8）
- [x] T037 `node/src/api/openai_endpoints.cpp` の chatCompletions() で n 回の生成ループ実装
- [x] T038 `node/src/api/openai_endpoints.cpp` の completions() で n 回の生成ループ実装

## Phase 3.4: 統合

- [x] T039 `node/src/api/openai_endpoints.cpp` ストリーミングレスポンスでのID・created対応
- [x] T040 `node/src/api/openai_endpoints.cpp` ストリーミングでの logprobs 対応（明示的非対応エラー）
- [x] T041 `node/src/api/openai_endpoints.cpp` n > 1 とストリーミング同時指定時の処理（明示的非対応エラー）

## Phase 3.5: 仕上げ

- [ ] T042 [P] `node/tests/contract/openai_api_test.cpp` 全テストがパスすることを確認
- [ ] T043 [P] `node/tests/integration/openai_endpoints_test.cpp` 全テストがパスすることを確認
- [ ] T044 既存の `make openai-tests` がパスすることを確認
- [ ] T045 [P] `specs/SPEC-24157000/spec.md` のステータスを「実装完了」に更新

## 依存関係

```text
T001, T002 (Setup)
    ↓
T003-T015 (Tests) - 並列実行可能
    ↓
T016-T018 (Utilities)
    ↓
T019-T024 (P1: usage, ID) - T016-T018に依存
    ↓
T025-T034 (P2: penalty, logprobs) - T019-T024と並列可能
    ↓
T035-T038 (P3: n parameter) - T019-T024に依存
    ↓
T039-T041 (Integration)
    ↓
T042-T045 (Polish)
```

## 並列実行例

```bash
# Phase 3.2: テスト作成を並列実行
Task: "T003 node/tests/contract/openai_api_test.cpp に ChatCompletionsReturnsUsage テスト"
Task: "T004 node/tests/contract/openai_api_test.cpp に CompletionsReturnsUsage テスト"
Task: "T005 node/tests/contract/openai_api_test.cpp に ResponseIdIsUnique テスト"
# ... 同一ファイルだが異なるテストケースなので順次追加

# Phase 3.3: ユーティリティ関数を順次実装
Task: "T016 count_tokens() 関数実装"
Task: "T017 generate_response_id() 関数実装"
Task: "T018 get_current_timestamp() 関数実装"
```

## 注意事項

- [P] タスク = 異なるファイル、依存関係なし
- 実装前にテストが失敗することを確認（TDD RED）
- 各タスク後にコミット推奨
- T003-T013は同一ファイルへの追加だが、テストケースが独立しているため順次追加
- llama.cpp APIの呼び出しにはモデルロード済みの前提が必要

## 検証チェックリスト

- [x] すべてのユーザーストーリー（P1-P3）に対応するテストがある
- [x] すべてのデータモデル（TokenUsage, LogprobInfo）に実装タスクがある
- [x] すべてのテストが実装より先にある（TDD順序）
- [x] 並列タスクは本当に独立している
- [x] 各タスクは正確なファイルパスを指定
- [x] 同じファイルを同時に変更する[P]タスクがない
