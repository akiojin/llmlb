# タスク: safetensors量子化対応

**機能ID**: SPEC-afa11b7d
**日付**: 2026-01-27
**仕様**: [spec.md](./spec.md) | **計画**: [plan.md](./plan.md)

## 凡例

- `[P]` - 並列実行可能
- `[ ]` - 未着手
- `[x]` - 完了
- 依存: 前提タスク番号

## Phase 1: 事前調査/設計確定

- [ ] T001 research.mdにsafetensors.cppの量子化方式一覧とバックエンド制約を整理
- [ ] T002 spec.mdのFR-007〜FR-009を確定（方式一覧/指定経路/フォールバック方針）
- [ ] T003 data-model.mdのQuantizationRequest/Statusを確定
- [ ] T004 contracts/cli.mdの表示ルール/エラー文言を確定

## Phase 2: US1 (P1) 量子化推論

### Tests First (RED)

- [ ] T010 [P] xllm/tests/unit/model_storage_test.cpp にsafetensors量子化指定の解決テストを追加 (依存: T002, T003)
- [ ] T011 [P] xllm/tests/unit/inference_engine_test.cpp に量子化指定の伝播/拒否テストを追加 (依存: T002, T003)

### Implementation (GREEN)

- [ ] T012 xllm/src/models/model_storage.cpp でsafetensors量子化指定を解決しmetadataへ反映 (依存: T010)
- [ ] T013 xllm/engines/safetensors/safetensors_engine.h/.cpp に量子化設定の受け渡しを追加 (依存: T011)
- [ ] T014 xllm/src/core/inference_engine.cpp で量子化指定の検証とエラー整形を追加 (依存: T011, T012)

## Phase 3: US2 (P1) 指定/表示

### Tests First (RED)

- [ ] T020 [P] xllm/tests/contract/cli_show_test.cpp に量子化表示の契約テストを追加 (依存: T004)
- [ ] T021 [P] xllm/tests/contract/cli_list_test.cpp に量子化判別表示の契約テストを追加 (依存: T004)

### Implementation (GREEN)

- [ ] T022 xllm/src/main.cpp の /api/show /api/tags レスポンスに量子化メタ情報を追加 (依存: T012)
- [ ] T023 xllm/src/cli/commands/show.cpp で量子化情報を出力 (依存: T020, T022)
- [ ] T024 xllm/src/cli/commands/list.cpp で量子化を判別できる表示へ更新 (依存: T021, T022)

## Phase 4: US3 (P2) 未対応量子化の明確なエラー

### Tests First (RED)

- [ ] T030 [P] xllm/tests/unit/inference_engine_test.cpp に未対応方式エラーのテストを追加 (依存: T002)
- [ ] T031 [P] xllm/tests/unit/model_storage_test.cpp に不正量子化指定のテストを追加 (依存: T002)

### Implementation (GREEN)

- [ ] T032 xllm/src/core/inference_engine.cpp で未対応/不正量子化のエラーを明確化 (依存: T030)
- [ ] T033 xllm/src/models/model_storage.cpp で不正指定の検出・返却を追加 (依存: T031)

## Phase 5: 仕上げ

- [ ] T040 quickstart.mdに量子化指定の具体手順を反映
- [ ] T041 spec.mdのステータス更新と要件の明確化反映
- [ ] T042 全体テスト実行（make quality-checks）
