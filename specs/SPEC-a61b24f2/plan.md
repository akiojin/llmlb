# 実装計画: モデル形式選択（safetensors/GGUF）とGGUF選択ポリシー

**機能ID**: `SPEC-a61b24f2`  
**作成日**: 2025-12-21  
**ステータス**: 実装済み  
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

登録時にモデル形式（safetensors/GGUF）を選択できるようにし、
- safetensors と GGUF が両方ある場合は `format` を必須化
- GGUF を選ぶ場合は品質/省メモリ/速度のポリシーで siblings から適切な量子化GGUFを選択
を実行できるようにする。ダッシュボードで選択と説明を表示し、READMEに手順を記載する。

## 影響範囲

- ルーターAPI: `/v0/models/register` のパラメータ拡張とバリデーション
- 変換パイプライン: 量子化の選択・後段量子化処理の追加
- ダッシュボードUI: 量子化セレクタと説明表示
- README: 量子化選択と外部ツールの要件を明記

## 設計方針

- `format` は `safetensors` / `gguf` を受け付ける
- HF上に両方存在する場合は `format` 未指定をエラーにする
- GGUF は `filename` が未指定の場合、`gguf_policy` に基づいて siblings を優先して選ぶ
- 非GGUF変換後の量子化は `llama-quantize` を利用
- 説明は登録ダイアログで明示（`format` と `gguf_policy`）

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

- API: `format` / `gguf_policy` 追加、siblings選択ロジック、バリデーション
- Conversion: GGUFポリシーの解釈、llama-quantize連携（必要時）、失敗時の明確なエラー
- UI: 形式セレクタ + GGUFポリシーセレクタ、説明表示、API連携
- Docs: README更新
