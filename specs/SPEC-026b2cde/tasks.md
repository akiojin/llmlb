# タスク: リクエスト履歴一覧のページネーション機能

**機能ID**: `SPEC-026b2cde`
**ステータス**: 完了
**入力**: `/specs/SPEC-026b2cde/` の設計ドキュメント

## 技術スタック

- **Backend**: Rust 1.75+ (Axum)
- **Frontend**: TypeScript 5.x (React)
- **Storage**: SQLite (request_history テーブル)
- **Tests**: cargo test, Vitest

## Phase 3.1: セットアップ

- [x] T001 `llmlb/src/api/dashboard.rs` に RequestHistoryQuery 構造体を定義
- [x] T002 ページサイズ正規化ロジック `normalized_per_page()` を実装

## Phase 3.2: テストファースト (TDD)

- [x] T003 [P] `llmlb/src/api/dashboard.rs` mod tests にページサイズ正規化テスト
- [x] T004 [P] `dashboard/` にページネーションコンポーネントのユニットテスト

## Phase 3.3: コア実装

- [x] T005 `llmlb/src/api/dashboard.rs` に GET /api/dashboard/request-history エンドポイント実装
  - page, per_page クエリパラメータ対応
  - 許可サイズ: [10, 25, 50, 100]
  - レスポンス: records, total, page, per_page, total_pages

- [x] T006 [P] `dashboard/src/components/` にページネーションUIコンポーネント
  - 表示件数ドロップダウン (10/25/50/100件)
  - 前へ/次へボタン
  - 現在ページ/総ページ数表示 (X / Y 形式)

- [x] T007 [P] `dashboard/src/pages/` リクエスト履歴ページにページネーション統合
  - 表示件数変更時の1ページ目リセット
  - フィルタ変更時の1ページ目リセット

## Phase 3.4: 統合

- [x] T008 ページネーション状態管理 (React state)
- [x] T009 APIとフロントエンドの連携確認
- [x] T010 エッジケース処理
  - 0件時: "- / -" 表示、ボタン無効
  - 表示件数以下: "1 / 1" 表示、ボタン無効

## Phase 3.5: 仕上げ

- [x] T011 パフォーマンス検証 (ページ切替 < 0.1秒)
- [x] T012 100件以上の履歴での動作確認
- [x] T013 フィルタとの連携確認

## 依存関係

```text
T001 → T005 (構造体定義 → API実装)
T003, T004 → T005, T006, T007 (テスト → 実装)
T005, T006, T007 → T008, T009, T010 (実装 → 統合)
T008, T009, T010 → T011, T012, T013 (統合 → 仕上げ)
```

## 並列実行例

```text
# Phase 3.2 テスト (並列実行可能)
Task T003: llmlb/src/api/dashboard.rs のページサイズ正規化テスト
Task T004: dashboard/ のページネーションコンポーネントテスト

# Phase 3.3 コア実装 (T006, T007 は並列実行可能)
Task T006: dashboard/src/components/ ページネーションUIコンポーネント
Task T007: dashboard/src/pages/ リクエスト履歴ページ統合
```

## 検証チェックリスト

- [x] すべてのユーザーストーリーに対応するタスクがある
- [x] すべてのテストが実装より先にある (TDD)
- [x] 並列タスクは本当に独立している
- [x] 各タスクは正確なファイルパスを指定
- [x] 同じファイルを変更する [P] タスクがない
