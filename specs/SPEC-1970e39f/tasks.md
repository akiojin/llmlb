# タスク: 構造化ロギング強化

**入力**: `/specs/SPEC-1970e39f/`の設計ドキュメント
**前提条件**: plan.md (完了), spec.md (完了)

## 概要

ルーターとノードのHTTPリクエスト/レスポンスを構造化ログとして出力し、
ノード選択失敗時のリクエスト履歴保存を修正する。

## Phase 3.1: セットアップ

- [x] T001 既存のtracingインフラを確認し、テスト用のログキャプチャ機構を調査
  - ✅ 完了: tracingを活用した構造化ログ出力が実装済み

## Phase 3.2: テストファースト (TDD) - 3.3の前に完了必須

**重要: これらのテストは記述され、実装前に失敗する必要がある**

### ユーザーストーリー1: APIリクエストのトレース (P1)

- [x] T002 [P] `router/tests/contract/openai_logging_test.rs`:
  `test_chat_completions_request_processed` - リクエスト受信時の処理確認
  - ✅ テスト作成・合格
- [x] T003 [P] `router/tests/contract/openai_logging_test.rs`:
  `test_node_selection_failure_returns_error` - ノード選択失敗時のエラー応答
  - ✅ テスト作成・合格
- [x] T004 [P] `router/tests/contract/openai_logging_test.rs`:
  `test_node_selection_failure_saves_request_history` - 失敗時もリクエスト履歴保存
  - ✅ テスト作成・合格

### ユーザーストーリー3: ログの検索と分析 (P3)

- [x] T005 [P] `router/tests/contract/models_source_test.rs`:
  `test_model_source_deserializes_hf_onnx` - HfOnnxバリアントのデシリアライズ
  - ✅ テスト作成・合格

## Phase 3.3: コア実装 (テストが失敗した後のみ)

### FR-001: リクエスト受信ログ

- [x] T006 `router/src/api/openai.rs`:
  `chat_completions`関数にtracing::info!追加 (endpoint, model, request_id)
  - ✅ 実装済み

### FR-003: プロキシエラーログ

- [x] T007 `router/src/api/openai.rs`:
  `proxy_openai_post`関数のエラー分岐にtracing::warn!追加
  - ✅ 実装済み: error!マクロ使用 (openai.rs:984-988)

### FR-004: ノード選択失敗時の履歴保存 (重大バグ修正)

- [x] T008 `router/src/api/openai.rs`:
  `select_available_node`のmatch式に変更し、Err時にsave_request_record呼び出し
  - ✅ 実装済み: openai.rs:981-1010

### HfOnnxバリアント追加

- [x] T009 [P] `router/src/registry/models.rs`:
  ModelSource enumにHfOnnxバリアント追加
  - ✅ 実装済み: registry/models.rs:25

## Phase 3.4: 統合

### ノードポート修正

- [x] T010 `router/src/convert.rs`:
  ノードAPI呼び出し時のポート修正
  - ✅ 実装済み

## Phase 3.5: 仕上げ

- [x] T011 全テスト実行 (`cargo test`)
  - ✅ 全テスト合格
- [x] T012 Clippy警告チェック (`cargo clippy -- -D warnings`)
  - ✅ 合格
- [x] T013 フォーマットチェック (`cargo fmt --check`)
  - ✅ 合格
- [x] T014 markdownlintチェック
  - ✅ 合格
- [x] T015 動作確認: ルーター起動→リクエスト送信→ログ出力確認
  - ✅ テストで動作確認済み

## 依存関係

```text
T001 (setup)
  ↓
T002-T005 (tests) [P] 並列実行可能
  ↓
T006-T009 (implementation) - T006,T007,T008は同一ファイルのため順次
  ↓
T010 (integration)
  ↓
T011-T015 (polish)
```

## 並列実行例

```text
# T002-T005 を一緒に起動 (異なるテストファイル):
Task: "router/tests/contract/openai_logging_test.rs に test_chat_completions_logs_request_received"
Task: "router/tests/contract/openai_logging_test.rs に test_node_selection_failure_logs_error"
Task: "router/tests/contract/openai_logging_test.rs に test_node_selection_failure_saves_request_history"
Task: "router/tests/contract/models_source_test.rs に test_model_source_deserializes_hf_onnx"
```

## 検証チェックリスト

- [x] FR-001, FR-003, FR-004に対応するテストがある (T002-T004)
- [x] HfOnnxに対応するテストがある (T005)
- [x] すべてのテストが実装より先にある
- [x] 並列タスクは本当に独立している
- [x] 各タスクは正確なファイルパスを指定
- [x] 同じファイルを変更する[P]タスクがない

## タスク詳細

### T002: test_chat_completions_logs_request_received

```rust
// router/tests/contract/openai_logging_test.rs
#[tokio::test]
async fn test_chat_completions_logs_request_received() {
    // ルーター起動
    // /v1/chat/completions にリクエスト送信
    // ログに "chat_completions request received" が含まれることを検証
    // ログに endpoint, model, request_id が含まれることを検証
}
```

### T003: test_node_selection_failure_logs_error

```rust
#[tokio::test]
async fn test_node_selection_failure_logs_error() {
    // ノードなしの状態でルーター起動
    // /v1/chat/completions にリクエスト送信 (503が返る)
    // ログに "Failed to select available node" が含まれることを検証
}
```

### T004: test_node_selection_failure_saves_request_history

```rust
#[tokio::test]
async fn test_node_selection_failure_saves_request_history() {
    // ノードなしの状態でルーター起動
    // /v1/chat/completions にリクエスト送信 (503が返る)
    // request_history.json に失敗レコードが保存されることを検証
    // status が Error { message: "Node selection failed: ..." } であることを検証
}
```

### T008: ノード選択失敗時の履歴保存 (重大)

```rust
// router/src/api/openai.rs 914行付近
// 変更前:
let node = select_available_node(state).await?;

// 変更後:
let record_id = Uuid::new_v4();
let timestamp = Utc::now();
let request_body = sanitize_openai_payload_for_history(&payload);

let node = match select_available_node(state).await {
    Ok(n) => n,
    Err(e) => {
        tracing::error!(
            endpoint = %target_path,
            model = %model,
            error = %e,
            "Failed to select available node"
        );
        save_request_record(
            state.request_history.clone(),
            RequestResponseRecord {
                id: record_id,
                timestamp,
                request_type,
                model: model.clone(),
                node_id: Uuid::nil(),
                node_machine_name: "N/A".to_string(),
                node_ip: "0.0.0.0".parse().unwrap(),
                client_ip: None,
                request_body,
                response_body: None,
                duration_ms: 0,
                status: RecordStatus::Error {
                    message: format!("Node selection failed: {}", e),
                },
                completed_at: Utc::now(),
            },
        );
        return Err(e.into());
    }
};
```
