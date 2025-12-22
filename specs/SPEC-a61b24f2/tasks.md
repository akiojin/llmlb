# タスク一覧: モデル形式選択（safetensors/GGUF）とGGUF選択ポリシー

**機能ID**: `SPEC-a61b24f2`

## 前提条件
- plan.md ✅

## タスク

### Setup
- [x] T001 形式（safetensors/GGUF）とGGUFポリシー（品質/省メモリ/速度）の固定リスト定義（API/UIで共通）

### Test (TDD: RED → GREEN → REFACTOR)
- [x] T010 [P] `router/tests/contract/models_api_test.rs` に「両方ある場合はformat必須」の契約テストを追加
- [x] T011 [P] `router/tests/contract/models_api_test.rs` にGGUFポリシー選択の契約テストを追加

### Core
- [x] T020 `/v0/models/register` に `format` と `gguf_policy` を追加しバリデーションを実装
- [x] T021 `format=gguf` + `filename` 未指定時の siblings 選択ロジック（ポリシー）を実装

### Integration
- [x] T030 ダッシュボード登録モーダルに `format` / `gguf_policy` セレクタを追加
- [x] T031 説明文（形式選択、GGUFポリシー）をダッシュボードに表示
- [x] T032 APIクライアントに `format` / `gguf_policy` パラメータを追加

### Polish
- [x] T040 README.md / README.ja.md に形式選択とGGUFポリシー、外部ツール要件を追記

## 次のステップ
- `/speckit.implement` または手動でタスクを実行
