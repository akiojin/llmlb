# 実装計画: GGUF量子化選択と量子化キャッシュ

**機能ID**: `SPEC-a61b24f2`  
**作成日**: 2025-12-21  
**ステータス**: 設計中  
**対象**: ルーターAPI / 変換パイプライン / ダッシュボード / README

## 実行フロー (/speckit.plan コマンドのスコープ)

1. 目的・要件確認
2. 既存仕様の参照（モデル登録・変換フロー）
3. 制約・依存関係整理
4. 設計方針の決定
5. 出力ファイルの確定
6. Phase 2 タスク分解方針の記述
7. 停止 - /speckit.tasks コマンドの準備完了

## 目的

量子化指定を登録時に受け取り、
- GGUFが存在する場合は siblings から該当量子化ファイルを選択
- 非GGUFの場合は変換後に llama-quantize により量子化
を実行できるようにする。ダッシュボードで選択と説明を表示し、READMEに手順を記載する。

## 影響範囲

- ルーターAPI: `/v0/models/register` のパラメータ拡張とバリデーション
- 変換パイプライン: 量子化の選択・後段量子化処理の追加
- ダッシュボードUI: 量子化セレクタと説明表示
- README: 量子化選択と外部ツールの要件を明記

## 設計方針

- 量子化タイプは固定リスト（UI/API双方で同じ）
- `filename` 未指定 + `quantization` 指定の場合は siblings を優先
- 量子化一致がない場合はエラー（変換へのフォールバックなし）
- 非GGUF変換後の量子化は `llama-quantize` を利用
- 変換/量子化の説明は登録ダイアログで明示

## 依存関係

- Hugging Face API (`expand=siblings`) の利用
- llama.cpp の `convert_hf_to_gguf.py`
- llama.cpp の `llama-quantize` バイナリ（環境変数で指定）

## 出力ファイル

- `router/src/api/models.rs`
- `router/src/convert.rs`
- `router/tests/contract/models_api_test.rs`
- `router/src/web/dashboard/src/components/models/ModelsSection.tsx`
- `router/src/web/dashboard/src/lib/api.ts`
- `README.md`, `README.ja.md`

## Phase 2: タスク分割方針（/speckit.tasksで具現化）

- API: 量子化パラメータ追加、siblings選択ロジック、バリデーション
- Conversion: 量子化指定の解釈、llama-quantize連携、失敗時の明確なエラー
- UI: 量子化セレクタ追加、説明表示、API連携
- Docs: README更新

