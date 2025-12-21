# タスク一覧: GGUF量子化選択と量子化キャッシュ

**機能ID**: `SPEC-a61b24f2`

## 前提条件
- plan.md ✅

## タスク

### Setup
- [x] T001 量子化タイプの固定リスト定義（API/UIで共通）

### Test (TDD: RED → GREEN → REFACTOR)
- [x] T010 [P] `router/tests/contract/models_api_test.rs` に量子化 siblings 選択の契約テストを追加
- [x] T011 [P] `router/tests/contract/models_api_test.rs` に量子化不一致/未発見のエラーテストを追加
- [x] T012 [P] `router/tests/contract/models_api_test.rs` にfilename指定時の量子化不一致テストを追加

### Core
- [x] T020 `/v0/models/register` に `quantization` を追加しバリデーションを実装
- [x] T021 `filename` 未指定 + `quantization` 指定時の siblings 選択ロジックを実装
- [x] T022 非GGUF変換時の量子化（llama-quantize連携）を実装

### Integration
- [x] T030 ダッシュボード登録モーダルに量子化セレクタを追加
- [x] T031 変換/量子化の説明文をダッシュボードに表示
- [x] T032 APIクライアントに `quantization` パラメータを追加

### Polish
- [x] T040 README.md / README.ja.md に量子化選択と外部ツール要件を追記

## 次のステップ
- `/speckit.implement` または手動でタスクを実行
