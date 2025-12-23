# タスク: モデル自動解決機能

**機能ID**: `SPEC-48678000`
**ステータス**: 計画中
**入力**: `/specs/SPEC-48678000/` の設計ドキュメント

## 技術スタック

- **Node**: C++17 (llama.cpp, httplib)
- **Router**: Rust 1.75+ (reqwest)
- **Storage**: ファイルシステム (`~/.llm-router/models/`)
- **Tests**: Google Test, cargo test

## Phase 3.1: セットアップ・クリーンアップ

- [ ] T001 `node/src/` から auto_repair 関連コードを特定・削除
- [ ] T002 auto_repair 関連の環境変数・設定を削除
- [ ] T003 関連するテストコードを削除

## Phase 3.2: テストファースト (TDD)

- [ ] T004 [P] `node/tests/` に共有パス参照の contract test
  - 共有パスにモデルがある場合の直接参照テスト
- [ ] T005 [P] `node/tests/` にルーターAPI経由ダウンロードの contract test
  - モックサーバーを使用したHTTP経由ダウンロードテスト
- [ ] T006 [P] `node/tests/` にモデル不在時のエラーハンドリング contract test
- [ ] T007 `node/tests/` に統合テスト: 解決フロー全体
  - ローカル → 共有パス → ルーターAPI → エラー のフォールバックフロー

## Phase 3.3: コア実装

- [ ] T008 `node/src/model_resolver.cpp` にモデル解決クラスを実装
  - ローカルキャッシュ確認
  - 共有パス直接参照（コピーなし）
  - ルーターAPI経由ダウンロード
  - エラーハンドリング

- [ ] T009 `node/src/model_resolver.cpp` に共有パス参照ロジック
  - NFSパスアクセス確認
  - ファイル存在チェック
  - 直接パス返却（コピーしない）

- [ ] T010 `node/src/model_resolver.cpp` にルーターAPI経由ダウンロード
  - `GET /v0/models/blob/:model_name` エンドポイント呼び出し
  - ローカルへの保存処理
  - 進捗表示（オプション）

- [ ] T011 `node/src/model_resolver.cpp` にエラーハンドリング
  - モデル未発見エラー
  - ネットワークエラー
  - ディスク容量不足エラー

## Phase 3.4: 統合

- [ ] T012 既存の推論フローに ModelResolver を統合
- [ ] T013 重複ダウンロード防止（ミューテックス）
- [ ] T014 設定ファイルから共有パス・ルーターURL読み込み

## Phase 3.5: 仕上げ

- [ ] T015 [P] `node/tests/` にユニットテスト追加
  - パス検証ロジック
  - エラーメッセージ生成
- [ ] T016 パフォーマンステスト: エラー応答 < 1秒
- [ ] T017 ドキュメント更新: モデル解決フローの説明

## 依存関係

```text
T001, T002, T003 → T004-T007 (クリーンアップ → テスト)
T004-T007 → T008-T011 (テスト → 実装)
T008 → T009, T010, T011 (基盤 → 詳細実装)
T008-T011 → T012-T014 (実装 → 統合)
T012-T014 → T015-T017 (統合 → 仕上げ)
```

## 並列実行例

```text
# Phase 3.2 テスト (並列実行可能)
Task T004: node/tests/ 共有パス参照 contract test
Task T005: node/tests/ ルーターAPI経由ダウンロード contract test
Task T006: node/tests/ モデル不在時エラー contract test
```

## 検証チェックリスト

- [ ] auto_repair 関連コードが完全に削除されている
- [ ] 共有パスからの直接参照でコピーが発生しない
- [ ] ルーターAPI経由ダウンロードが正常に動作する
- [ ] モデル不在時に1秒以内にエラーが返る
- [ ] Hugging Face への直接ダウンロードが禁止されている
- [ ] すべてのテストが実装より先にある (TDD)
