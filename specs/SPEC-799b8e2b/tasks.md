# 共通ログシステム タスク一覧

## Setup

- [x] SPEC-799b8e2bディレクトリ作成
- [x] spec.md作成
- [x] plan.md作成
- [x] tasks.md作成

## Load Balancer側

### Test (TDD Red)

- [x] ログファイル日付ローテーションのテスト作成
- [x] 古いログファイル削除のテスト作成
- [x] ログフォーマット（category含む）のテスト作成
- [x] 環境変数設定のテスト作成

### Core (TDD Green)

- [x] [P] ログディレクトリを`~/.llmlb/logs/`に変更
- [x] [P] ファイル名を`llmlb.jsonl.YYYY-MM-DD`に変更
- [x] 日付ベースローテーション実装
- [x] 7日超の古いファイル削除実装
- [x] 新環境変数サポート（LLM_LOG_DIR, LLM_LOG_LEVEL, LLM_LOG_RETENTION_DAYS）
- [x] categoryフィールド出力対応
- [x] [P] stdout出力追加（人間が読みやすい形式）

## Node側

### Test (TDD Red)

- [x] ログファイル日付ローテーションのテスト作成
- [x] 古いログファイル削除のテスト作成
- [x] ログフォーマット（category含む）のテスト作成
- [x] 環境変数設定のテスト作成

### Core (TDD Green)

- [x] [P] ログディレクトリを`~/.llmlb/logs/`に変更
- [x] [P] ファイル名を`xllm.jsonl.YYYY-MM-DD`に変更
- [x] [P] stdout出力追加（人間が読みやすい形式）
- [x] 日付ベースローテーション実装（daily_file_sink）
- [x] 7日超の古いファイル削除実装
- [x] 新環境変数サポート
- [x] categoryフィールド出力対応

## Integration

- [x] Load Balancer/Node同時起動でログ出力確認（手動検証）
  - ✅ 2025-12-28 実行: `LLMLB_LOG_DIR`/`XLLM_LOG_DIR` を指定して同時起動
  - ✅ ログ出力確認: `/tmp/llm-logs-799b8e2b.Qa6okk/llmlb.jsonl.2025-12-28`
  - ✅ ログ出力確認: `/tmp/llm-logs-799b8e2b.Qa6okk/xllm.jsonl.2025-12-28`
- [x] /v0/logsエンドポイント動作確認
  - ✅ SPEC-1f2a9c3d (Log Retrieval API) で実装・テスト済み
  - ✅ `llmlb/src/api/logs.rs` にwiremockテスト

## Polish

- [x] コードレビュー
  - ✅ 2025-12-25 確認完了
- [x] ドキュメント更新（README.md）
  - ✅ LLM_LOG_DIR, LLM_LOG_LEVEL, LLM_LOG_RETENTION_DAYS を README.md に記載
- [x] 品質チェック実行
