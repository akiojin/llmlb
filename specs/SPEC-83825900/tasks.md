# タスク: Nemotron CUDA PoC

**入力**: `/specs/SPEC-83825900/`の設計ドキュメント
**前提条件**: plan.md, research.md

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- 説明には正確なファイルパスを含める

## Phase 3.1: セットアップ

- [ ] T001 `poc/nemotron-cuda-cpp/` ディレクトリ構造を作成（src/, include/, test/）
- [ ] T002 `poc/nemotron-cuda-cpp/CMakeLists.txt` にCUDA対応ビルド設定を作成
- [ ] T003 `poc/nemotron-safetensors-cpp/safetensors.hh` を `poc/nemotron-cuda-cpp/include/` にコピー
- [ ] T004 [P] `poc/nemotron-cuda-cpp/include/config.h` に共通定義（エラーマクロ、型定義）を作成
- [ ] T005 [P] `poc/nemotron-cuda-cpp/include/cuda_utils.h` にCUDAエラーチェックマクロを作成

## Phase 3.2: コアローダー

- [ ] T006 `poc/nemotron-cuda-cpp/include/model_config.h` にconfig.json解析用構造体を定義
- [ ] T007 `poc/nemotron-cuda-cpp/src/model_config.cpp` にconfig.json読み込み実装
- [ ] T008 `poc/nemotron-cuda-cpp/include/safetensors_loader.h` にsafetensorsローダーインターフェースを定義
- [ ] T009 `poc/nemotron-cuda-cpp/src/safetensors_loader.cpp` にsafetensors mmapロード実装（既存PoCベース）
- [ ] T010 `poc/nemotron-cuda-cpp/include/cuda_memory.h` にCUDAメモリ管理インターフェースを定義
- [ ] T011 `poc/nemotron-cuda-cpp/src/cuda_memory.cu` にGPUメモリ確保・転送実装

## Phase 3.3: CUDAカーネル

- [ ] T012 [P] `poc/nemotron-cuda-cpp/include/kernels.h` にカーネル関数宣言を定義
- [ ] T013 [P] `poc/nemotron-cuda-cpp/src/kernels/rms_norm.cu` にRMSNormカーネルを実装
- [ ] T014 [P] `poc/nemotron-cuda-cpp/src/kernels/silu.cu` にSiLU活性化カーネルを実装
- [ ] T015 [P] `poc/nemotron-cuda-cpp/src/kernels/softmax.cu` にSoftmaxカーネルを実装
- [ ] T016 [P] `poc/nemotron-cuda-cpp/src/kernels/embedding.cu` にEmbedding Lookupカーネルを実装
- [ ] T017 `poc/nemotron-cuda-cpp/src/kernels/attention.cu` にScaled Dot-Product Attentionカーネルを実装（RoPE含む）

## Phase 3.4: トークナイザー

- [ ] T018 `poc/nemotron-cuda-cpp/include/tokenizer.h` にトークナイザーインターフェースを定義
- [ ] T019 `poc/nemotron-cuda-cpp/src/tokenizer.cpp` に簡易BPEトークナイザー実装（tokenizer.json読み込み）

## Phase 3.5: Transformerレイヤー

- [ ] T020 `poc/nemotron-cuda-cpp/include/transformer.h` にTransformerレイヤーインターフェースを定義
- [ ] T021 `poc/nemotron-cuda-cpp/src/transformer.cu` にTransformerレイヤー実装（Attention + MLP）
- [ ] T022 `poc/nemotron-cuda-cpp/src/transformer.cu` にcuBLAS GEMM呼び出し統合

## Phase 3.6: 推論ループ

- [ ] T023 `poc/nemotron-cuda-cpp/include/inference.h` に推論インターフェースを定義
- [ ] T024 `poc/nemotron-cuda-cpp/src/inference.cpp` にモデルロード処理を実装
- [ ] T025 `poc/nemotron-cuda-cpp/src/inference.cpp` に生成ループ（autoregressive）を実装
- [ ] T026 `poc/nemotron-cuda-cpp/src/inference.cpp` にサンプリング（greedy/top-k）を実装

## Phase 3.7: メインエントリポイント

- [ ] T027 `poc/nemotron-cuda-cpp/src/main.cpp` にCLI引数パース（--model, --prompt, --max-tokens）を実装
- [ ] T028 `poc/nemotron-cuda-cpp/src/main.cpp` にロード時間・生成速度の計測を実装
- [ ] T029 ビルドテスト: CMakeでビルドが通ることを確認

## Phase 3.8: 統合テスト（Nemotron-Mini）

- [ ] T030 Nemotron-Miniモデルをダウンロード（HuggingFace）
- [ ] T031 `./nemotron-cuda-poc --model <path> --prompt "Hello"` で1トークン生成を確認
- [ ] T032 複数トークン生成（--max-tokens 100）を確認
- [ ] T033 エラーケーステスト: 不正パス、CUDA未対応環境

## Phase 3.9: 拡張検証（Nemotron-Medium）

- [ ] T034 Nemotron-Mediumモデルをダウンロード（24GB+ GPU必要）
- [ ] T035 Nemotron-Mediumで推論実行・性能測定

## Phase 3.10: ドキュメント

- [ ] T036 [P] `poc/nemotron-cuda-cpp/README.md` にビルド手順を記載
- [ ] T037 [P] `poc/nemotron-cuda-cpp/README.md` に実行例・オプション説明を追記

## 依存関係

```text
T001 → T002 → T003 → (T004, T005)
T006 → T007
T008 → T009
T010 → T011
(T009, T011) → T012 → (T013, T014, T015, T016) → T017
T018 → T019
(T017, T019) → T020 → T021 → T022
T022 → T023 → T024 → T025 → T026
T026 → T027 → T028 → T029
T029 → T030 → T031 → T032 → T033
T033 → T034 → T035
(T035) → (T036, T037)
```

## 並列実行例

```bash
# Phase 3.1 並列タスク:
Task: "poc/nemotron-cuda-cpp/include/config.h に共通定義を作成"
Task: "poc/nemotron-cuda-cpp/include/cuda_utils.h にCUDAエラーチェックマクロを作成"

# Phase 3.3 並列タスク:
Task: "poc/nemotron-cuda-cpp/src/kernels/rms_norm.cu にRMSNormカーネルを実装"
Task: "poc/nemotron-cuda-cpp/src/kernels/silu.cu にSiLU活性化カーネルを実装"
Task: "poc/nemotron-cuda-cpp/src/kernels/softmax.cu にSoftmaxカーネルを実装"
Task: "poc/nemotron-cuda-cpp/src/kernels/embedding.cu にEmbedding Lookupカーネルを実装"
```

## 注意事項

- [P] タスク = 異なるファイル、依存関係なし
- 各タスク後にコミット推奨（PoCのため柔軟に）
- CUDAビルドにはCUDA Toolkit 12.x必須
- 回避: 曖昧なタスク、同じファイルの競合

## 検証チェックリスト

- [ ] CMakeビルドが通る（T029）
- [ ] Nemotron-Miniで1トークン生成成功（T031）
- [ ] 複数トークン生成成功（T032）
- [ ] エラーメッセージが明確（T033）
- [ ] ロード時間・生成速度が表示される（T028）
