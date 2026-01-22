# タスク: Open Responses API対応

**入力**: `/specs/SPEC-99024000/`の設計ドキュメント
**前提条件**: plan.md (必須), spec.md

## 実装状況

**既存実装（SPEC-24157000で完了）**:

- `llmlb/src/api/responses.rs` - `/v1/responses`ハンドラー
- `llmlb/src/types/endpoint.rs` - `SupportedAPI`列挙型、`supports_responses_api`フラグ
- `llmlb/src/api/mod.rs` - ルート登録済み（234行目）
- `llmlb/src/api/proxy.rs` - `forward_to_endpoint`、`forward_streaming_response`

**テスト実装完了**: 2026-01-16

## Phase 3.1: セットアップ

- [x] T001 既存実装の確認（SPEC-24157000で完了）
- [x] T002 統合テストファイル確認: `llmlb/tests/integration/responses_api_test.rs`

## Phase 3.2: テストファースト (TDD) ⚠️ 3.3の前に完了必須

**重要: これらのテストは記述され、既存実装の動作を検証する**

- [x] T003 [P] `llmlb/tests/integration/responses_api_test.rs` に
  RES001: Responses API対応バックエンドへのリクエスト転送テスト
  - `res001_responses_passthrough_preserves_request_body`
  - `res001_responses_passthrough_with_tools`

- [x] T004 [P] `llmlb/tests/integration/responses_streaming_test.rs` に
  RES002: ストリーミングリクエストのパススルーテスト
  - `responses_streaming_passthrough_events`
  - `responses_streaming_events_preserve_order`
  - `responses_streaming_collects_full_text`

- [x] T005 [P] `llmlb/tests/integration/responses_api_test.rs` に
  RES003: 非対応バックエンドへの501エラーテスト
  - `res003_non_supporting_backend_returns_501`

- [x] T006 [P] `llmlb/tests/integration/responses_api_test.rs` に
  RES004: 認証なしリクエストへの401エラーテスト
  - `res004_request_without_auth_returns_401`

- [x] T007 [P] `llmlb/tests/integration/responses_api_test.rs` に
  RES005: ルート存在確認テスト
  - `res005_responses_route_exists`

## Phase 3.3: 検証・修正

- [x] T008 統合テスト実行: `cargo test --test integration_tests responses`
  - すべてのテストがパス（GREEN）: 11テスト成功

- [x] T009 `/v1/models`レスポンスにAPI対応情報が含まれるか検証
  - `models_api_includes_supported_apis_field` テストで検証済み
  - `models_api_shows_chat_only_for_non_responses_backend` テストで検証済み

- [x] T010 ヘルスチェックでResponses API対応検出を検証
  - `register_responses_endpoint` がヘルスチェック＋モデル同期を実行
  - `supports_responses_api` フラグが正しく設定されることを確認

## Phase 3.4: 仕上げ

- [x] T011 [P] 全統合テスト実行: `cargo test --test integration_tests`
  - 62テスト成功、回帰なし

- [x] T012 [P] 品質チェック
  - `cargo fmt --check` ✓
  - `cargo clippy -- -D warnings` ✓

- [x] T013 spec.mdのステータスを「完了」に更新

- [x] T014 plan.mdの進捗を更新

## 依存関係

```text
T002 → T003-T007 (テストファイル作成が先) ✓
T003-T007 → T008 (テスト作成後に実行) ✓
T008 → T009-T010 (テスト合格後に検証) ✓
T009-T010 → T011-T014 (検証後に仕上げ) ✓
```

## テスト結果サマリー

| テストファイル | テスト数 | 結果 |
|---------------|---------|------|
| responses_api_test.rs | 7 | ✓ PASS |
| responses_streaming_test.rs | 3 | ✓ PASS |
| models_api_test.rs (関連) | 1 | ✓ PASS |
| **合計** | **11** | **✓ ALL PASS** |

## 検証チェックリスト

- [x] すべての統合テストシナリオ（RES001-RES005）にテストがある
- [x] テストが既存実装の動作を正しく検証している
- [x] 並列タスクは本当に独立している
- [x] 各タスクは正確なファイルパスを指定
- [x] 品質チェックがすべてパス
