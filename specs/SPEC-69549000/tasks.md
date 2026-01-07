# Tasks: safetensors.cpp

**機能ID**: SPEC-69549000
**日付**: 2026-01-06
**仕様**: [spec.md](./spec.md) | **計画**: [plan.md](./plan.md)

## 凡例

- `[P]` - 並列実行可能
- `[ ]` - 未着手
- `[x]` - 完了
- 依存: 前提タスク番号

## Phase 1: 基盤構築

### Setup

- [x] 1. プロジェクトディレクトリ構造作成 `[P]`
  - `node/third_party/safetensors.cpp/` 配下に構造を作成
  - `include/`, `src/`, `examples/`, `tests/` ディレクトリ

- [ ] 2. ggmlサブモジュール追加 `[P]` (Worktree環境のため手動実行が必要)
  - `git submodule add https://github.com/ggml-org/ggml ggml`
  - `.gitmodules` 更新
  - Note: CMakeLists.txtは親プロジェクトのggmlを利用する設定で対応済み

- [x] 3. CMakeLists.txt作成 (依存: 1, 2)
  - ggmlサブディレクトリ追加
  - GPUバックエンド選択オプション（STCPP_METAL, STCPP_CUDA等）
  - ライブラリターゲット定義

- [x] 4. LICENSE (MIT) 作成 `[P]`

- [x] 5. README.md作成 `[P]`
  - ビルド手順
  - 使用例
  - 対応GPU

### Contract Tests

- [x] 6. テストフレームワーク設定 (依存: 3)
  - googletestを追加
  - CTest統合

- [x] 7. C API契約テスト: 初期化/終了 (依存: 6) `[P]`
  - `stcpp_init()` / `stcpp_free()` テスト
  - `stcpp_version()` / `stcpp_abi_version()` テスト

- [x] 8. C API契約テスト: モデル操作 (依存: 6) `[P]`
  - `stcpp_model_load()` / `stcpp_model_free()` テスト
  - エラーケース（ファイル不在、無効モデル）

- [x] 9. C API契約テスト: コンテキスト操作 (依存: 6) `[P]`
  - `stcpp_context_new()` / `stcpp_context_free()` テスト
  - `stcpp_context_default_params()` テスト

- [x] 10. C API契約テスト: トークナイザー (依存: 6) `[P]`
  - `stcpp_tokenize()` / `stcpp_detokenize()` テスト

## Phase 2: safetensorsローダー

### Tests First (RED)

- [x] 11. safetensorsパーサーテスト (依存: 6)
  - ヘッダー解析テスト
  - テンソルメタデータ読み込みテスト
  - 無効ファイルエラーテスト

- [x] 12. テンソル変換テスト (依存: 6)
  - safetensors → ggml_tensor変換テスト
  - FP16/BF16対応テスト

- [x] 13. 分割ファイルローダーテスト (依存: 6)
  - index.json解析テスト
  - 複数ファイル結合テスト

### Implementation (GREEN)

- [x] 14. safetensors.hヘッダー作成 (依存: 3)
  - contracts/c-api.mdに基づく型定義
  - 関数宣言

- [x] 15. safetensors-loader.cpp実装 (依存: 11, 14)
  - stable-diffusion.cppの実装を参考に移植
  - mmapサポート
  - ヘッダー解析
  - テンソル読み込み

- [x] 16. 分割ファイルローダー実装 (依存: 13, 15)
  - index.json解析
  - 複数ファイル結合

- [x] 17. テンソル変換実装 (依存: 12, 15)
  - safetensors → ggml_tensor変換
  - dtype対応（FP16, BF16）

## Phase 3: トークナイザー

### Tests First (RED)

- [x] 18. tokenizer.json解析テスト (依存: 6)
  - vocab読み込みテスト
  - BPEマージルール読み込みテスト
  - 特殊トークン読み込みテスト

- [x] 19. トークン化テスト (依存: 6)
  - 基本テキストのトークン化
  - 特殊トークン追加
  - エッジケース（空文字、長文）

- [x] 20. チャットテンプレートテスト (依存: 6)
  - Jinja2テンプレート解析
  - メッセージフォーマット適用

### Implementation (GREEN)

- [x] 21. tokenizer.cpp実装 (依存: 18, 19, 14)
  - tokenizer.json解析
  - BPEエンコード/デコード
  - llama.cppのトークナイザー参考

- [x] 22. chat-template.cpp実装 (依存: 20, 14)
  - Jinja2サブセット解析
  - テンプレート適用
  - llama.cppのテンプレートエンジン参考

## Phase 4: 推論エンジン（MVP）

### Tests First (RED)

- [x] 23. モデルロード統合テスト (依存: 17, 21)
  - config.json読み込みテスト
  - 完全なモデルロードテスト
  - VRAM見積もりテスト

- [x] 24. 生成テスト (依存: 6)
  - 単一トークン生成テスト
  - サンプリングパラメータテスト
  - ストリーミングコールバックテスト

- [x] 25. キャンセルテスト (依存: 6)
  - 推論キャンセルテスト
  - 部分結果取得テスト

### Implementation (GREEN)

- [x] 26. config.jsonローダー実装 (依存: 23, 14)
  - ModelConfig構造体パース
  - アーキテクチャ検出

- [x] 27. ggml compute graph実装 (依存: 26)
  - src/ggml_model.h, src/ggml_model.cpp - ggmlモデル構造体
  - src/transformer.cpp - Transformerレイヤー・計算グラフ構築
  - ggmlテンソル操作・KVキャッシュ

- [x] 28. sampling.cpp実装 (依存: 24, 14)
  - temperature, top_p, top_k
  - repeat_penalty
  - llama.cppのサンプリング参考

- [x] 29. safetensors.cpp コア実装 (依存: 27, 28)
  - stcpp_model_load()
  - stcpp_context_new()
  - stcpp_generate()
  - stcpp_generate_stream()

- [x] 30. キャンセル機能実装 (依存: 25, 29)
  - stcpp_cancel()
  - アトミックフラグによる中断

## Phase 5: KVキャッシュ

### Tests First (RED)

- [x] 31. KVキャッシュテスト (依存: 29)
  - キャッシュ割り当てテスト
  - キャッシュクリアテスト
  - 量子化テスト（INT8/FP8）

- [x] 32. プロンプトキャッシュテスト (依存: 29)
  - キャッシュ保存テスト
  - キャッシュ読み込みテスト
  - 再利用効果測定

### Implementation (GREEN)

- [x] 33. kv-cache.cpp実装 (依存: 31)
  - KVキャッシュ管理
  - INT8/FP8量子化
  - llama.cppのKVキャッシュ参考

- [x] 34. プロンプトキャッシュ実装 (依存: 32, 33)
  - stcpp_prompt_cache_save()
  - stcpp_prompt_cache_load()

## Phase 6: バッチ処理

### Tests First (RED)

- [x] 35. バッチ処理テスト (依存: 29)
  - 複数リクエスト追加テスト
  - バッチデコードテスト
  - リクエストキャンセルテスト

### Implementation (GREEN)

- [x] 36. batch.cpp実装 (依存: 35)
  - stcpp_batch_new()
  - stcpp_batch_add()
  - stcpp_batch_decode()
  - continuous batching

## Phase 7: 埋め込み

### Tests First (RED)

- [x] 37. 埋め込みテスト (依存: 29)
  - 埋め込み生成テスト
  - 次元数確認テスト
  - バッチ埋め込みテスト

### Implementation (GREEN)

- [x] 38. 埋め込み実装 (依存: 37)
  - stcpp_embeddings()
  - stcpp_embeddings_dims()

## Phase 8: LoRA

### Tests First (RED)

- [x] 39. LoRAテスト (依存: 29)
  - LoRAロードテスト
  - LoRA適用テスト
  - ホットリロードテスト

### Implementation (GREEN)

- [x] 40. lora.cpp実装 (依存: 39)
  - stcpp_lora_load()
  - stcpp_lora_apply()
  - stcpp_lora_remove()

## Phase 9: 追加アーキテクチャ

- [x] 41. ~~nemotronアーキテクチャテスト~~ (スコープ外: gpt-oss/nemotronエンジン削除により不要)
- [x] 42. ~~nemotronアーキテクチャ実装~~ (スコープ外: gpt-oss/nemotronエンジン削除により不要)

## Phase 10: 高度な機能

- [x] 43. Rope Scalingテスト (依存: 29)
- [x] 44. Rope Scaling実装 (依存: 43)
  - Linear/NTKスケーリング

- [x] 45. Sliding Window Attentionテスト (依存: 29)
- [x] 46. Sliding Window Attention実装 (依存: 45)

- [x] 47. GQA/MQAテスト (依存: 29)
- [x] 48. GQA/MQA実装 (依存: 47)

## Phase 11: マルチGPU

- [x] 49. マルチGPUテスト (依存: 29)
  - Pipeline Parallelismテスト
  - デバイス間通信テスト

- [x] 50. マルチGPU実装 (依存: 49)
  - レイヤー分割
  - デバイス間同期

## Phase 12: 仕上げ

### Integration Tests

- [x] 51. E2Eテスト: safetensorsモデル (依存: 29)
  - 完全なモデルロード→推論→結果検証
  - HuggingFace Tinyモデル（例: TinyLlama）使用

- [x] 52. E2Eテスト: ストリーミング (依存: 51)

- [x] 53. E2Eテスト: continuous batching (依存: 36, 51)

### Documentation

- [x] 54. APIリファレンス作成 (依存: 29)
- [x] 55. チュートリアル作成 (依存: 51) ※quickstart含むREADMEで対応
- [x] 56. サンプルコード充実 (依存: 51) ※examples/benchmark.cppで対応

### Performance

- [x] 57. ベンチマークツール作成 (依存: 51)
- [x] 58. HuggingFace transformers比較ベンチマーク (依存: 57)
  - `benchmarks/hf_benchmark.py` - HuggingFace transformersベンチマーク
  - `benchmarks/compare.py` - 結果比較ツール
  - `examples/benchmark.cpp` - JSON出力機能追加
- [ ] 59. VRAM使用量最適化 (依存: 58)

### CI/CD

- [x] 60. CIワークフロー作成 (依存: 51) `[P]`
  - ビルドテスト
  - ユニットテスト
  - HuggingFace Tinyモデルでの統合テスト

## 依存関係グラフ（主要パス）

```text
1,2 → 3 → 6 → 7,8,9,10 (契約テスト)
                ↓
            11,12,13 → 15,16,17 (ローダー)
                          ↓
                      18,19,20 → 21,22 (トークナイザー)
                                   ↓
                               23,24,25 → 26,27,28,29 (推論MVP)
                                              ↓
                                          31,32 → 33,34 (KVキャッシュ)
                                              ↓
                                          35 → 36 (バッチ)
                                              ↓
                                          51,52,53 (E2E)
```

## MVP完了条件

以下のタスクが完了した時点でMVP達成:

- [x] Phase 1: 基盤構築 (1-10)
- [x] Phase 2: safetensorsローダー (11-17)
- [x] Phase 3: トークナイザー (18-22)
- [x] Phase 4: 推論エンジン (23-30)
- [x] Phase 5: KVキャッシュ (31-34)
- [x] Phase 6: バッチ処理 (35-36)
- [x] Phase 7: 埋め込み (37-38)
- [x] Phase 8: LoRA (39-40)
- [x] Phase 9: 追加アーキテクチャ (41-42) ※gpt-oss/nemotronエンジン削除によりスコープ外
- [x] Phase 10: 高度な機能 (43-48)
- [x] Phase 11: マルチGPU (49-50)
- [x] E2Eテスト: safetensorsモデル (51)
- [x] E2Eテスト: ストリーミング (52)
- [x] E2Eテスト: continuous batching (53)

MVP = 単一GPUでのsafetensorsモデル推論 + ストリーミング出力

Note: gpt-oss/nemotronエンジンは`refactor!: remove gptoss/nemotron engines for safetensors.cpp migration`で削除済み。safetensors.cppはggmlバックエンドを直接使用するアーキテクチャ非依存の設計。
