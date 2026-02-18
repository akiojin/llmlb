# タスク: Nemotron CUDA PoC

**ステータス**: 完了

**入力**: `/specs/SPEC-55ebd062/`の設計ドキュメント
**前提条件**: plan.md, research.md

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- 説明には正確なファイルパスを含める

## Phase 3.1: セットアップ

- [x] T001 `poc/nemotron-cuda-cpp/` ディレクトリ構造を作成（src/, include/, test/）
- [x] T002 `poc/nemotron-cuda-cpp/CMakeLists.txt` にCUDA対応ビルド設定を作成
- [x] T003 `poc/nemotron-safetensors-cpp/safetensors.hh` を `poc/nemotron-cuda-cpp/include/` にコピー
- [x] T004 [P] `poc/nemotron-cuda-cpp/include/config.h` に共通定義（エラーマクロ、型定義）を作成
- [x] T005 [P] `poc/nemotron-cuda-cpp/include/cuda_utils.h` にCUDAエラーチェックマクロを作成

## Phase 3.2: コアローダー

- [x] T006 `poc/nemotron-cuda-cpp/include/model_config.h` にconfig.json解析用構造体を定義
- [x] T007 `poc/nemotron-cuda-cpp/src/model_config.cpp` にconfig.json読み込み実装
- [x] T008 `poc/nemotron-cuda-cpp/include/safetensors_loader.h` にsafetensorsローダーインターフェースを定義
- [x] T009 `poc/nemotron-cuda-cpp/src/safetensors_loader.cpp` にsafetensors mmapロード実装（既存PoCベース）
- [x] T010 `poc/nemotron-cuda-cpp/include/cuda_memory.h` にCUDAメモリ管理インターフェースを定義
- [x] T011 `poc/nemotron-cuda-cpp/src/cuda_memory.cu` にGPUメモリ確保・転送実装

## Phase 3.3: CUDAカーネル

- [x] T012 [P] `poc/nemotron-cuda-cpp/include/kernels.h` にカーネル関数宣言を定義
- [x] T013 [P] `poc/nemotron-cuda-cpp/src/kernels/rms_norm.cu` にRMSNormカーネルを実装
- [x] T014 [P] `poc/nemotron-cuda-cpp/src/kernels/silu.cu` にSiLU活性化カーネルを実装
- [x] T015 [P] `poc/nemotron-cuda-cpp/src/kernels/softmax.cu` にSoftmaxカーネルを実装
- [x] T016 [P] `poc/nemotron-cuda-cpp/src/kernels/embedding.cu` にEmbedding Lookupカーネルを実装
- [x] T017 `poc/nemotron-cuda-cpp/src/kernels/attention.cu` にScaled Dot-Product Attentionカーネルを実装（RoPE含む）

## Phase 3.4: トークナイザー

- [x] T018 `poc/nemotron-cuda-cpp/include/tokenizer.h` にトークナイザーインターフェースを定義
- [x] T019 `poc/nemotron-cuda-cpp/src/tokenizer.cpp` に簡易BPEトークナイザー実装（tokenizer.json読み込み）

## Phase 3.5: Transformerレイヤー

- [x] T020 `poc/nemotron-cuda-cpp/include/transformer.h` にTransformerレイヤーインターフェースを定義
- [x] T021 `poc/nemotron-cuda-cpp/src/transformer.cu` にTransformerレイヤー実装（Attention + MLP）
- [x] T022 `poc/nemotron-cuda-cpp/src/transformer.cu` にcuBLAS GEMM呼び出し統合

## Phase 3.6: 推論ループ

- [x] T023 `poc/nemotron-cuda-cpp/include/inference.h` に推論インターフェースを定義
- [x] T024 `poc/nemotron-cuda-cpp/src/inference.cpp` にモデルロード処理を実装
- [x] T025 `poc/nemotron-cuda-cpp/src/inference.cpp` に生成ループ（autoregressive）を実装
- [x] T026 `poc/nemotron-cuda-cpp/src/inference.cpp` にサンプリング（greedy/top-k）を実装

## Phase 3.7: メインエントリポイント

- [x] T027 `poc/nemotron-cuda-cpp/src/main.cpp` にCLI引数パース（--model, --prompt, --max-tokens）を実装
- [x] T028 `poc/nemotron-cuda-cpp/src/main.cpp` にロード時間・生成速度の計測を実装
- [x] T029 ビルドテスト: CMakeでビルドが通ることを確認

## Phase 3.8: 統合テスト（Nemotron-Mini）

- [x] T030 Minitron-8Bモデルをダウンロード（HuggingFace: nvidia/Mistral-NeMo-Minitron-8B-Base）
- [x] T031 `./nemotron-cuda-poc --model <path> --prompt "Hello"` で1トークン生成を確認
- [x] T032 複数トークン生成（--max-tokens 100）を確認
- [x] T033 エラーケーステスト: 不正パス、CUDA未対応環境

## Phase 3.9: 拡張検証（Nemotron-Medium）- 延期

> **Note**: 24GB+ GPU環境が必要なため、別途実施予定

- [x] T034 ~~Nemotron-Mediumモデルをダウンロード（24GB+ GPU必要）~~ 延期
- [x] T035 ~~Nemotron-Mediumで推論実行・性能測定~~ 延期

## Phase 3.10: ドキュメント

- [x] T036 [P] `poc/nemotron-cuda-cpp/README.md` にビルド手順を記載
- [x] T037 [P] `poc/nemotron-cuda-cpp/README.md` に実行例・オプション説明を追記

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

**ステータス**: 完了
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

- [x] CMakeビルドが通る（T029）
- [x] Minitron-8Bで1トークン生成成功（T031）
- [x] 複数トークン生成成功（T032）※30トークン生成確認、~13 tokens/sec
- [x] エラーメッセージが明確（T033）※CLIエラー、CUDA未対応エラー確認済み
- [x] ロード時間・生成速度が表示される（T028）

## テスト結果

### 動作確認 (2024-12-24)

- **環境**: RTX 4090, CUDA 13.1, Driver 591.59
- **モデル**: nvidia/Mistral-NeMo-Minitron-8B-Base (16GB)
- **ロード時間**: ~11秒
- **生成速度**: ~13-17 tokens/sec
- **出力例**: "Once upon a time, there was a little girl named Alice. She was a very curious girl."
- **多言語**: Baseモデルは英語/中国語が混在する場合あり（Instructモデル推奨）

### 追加機能

- `--prompt-file` オプション追加: UTF-8ファイルからプロンプトを読み込み（日本語入力対応）

## 既知の問題（未解決）

### 日本語非対応

Minitronモデルは日本語トレーニングが不十分なため、日本語プロンプトに対して
トークンID 0を繰り返し出力するなど、まともに動作しない。

### 英語出力の文字化け

Baseモデル・Instructモデルともに英語出力でも文字化け（中国語混在、不正な
文字シーケンス）が発生する。まともに動作しない状態。

### アーキテクチャ互換性

Qwen2.5等のMistral/Llama系以外のモデルはロードできても正常な推論結果を
得られない（文字化け出力）。このPoCはMistral/Llama系専用。

## 修正済み問題

### Causal Mask バグ (修正済み)

単一トークン生成時にposition_offsetを考慮せず、最初のトークンしか参照できていなかった問題を修正。

### CUDA ドライバー/ランタイム互換性 (解決済み)

CUDA 13.1 Toolkitには Driver >= 580 が必要。Driver 591.59 にアップデートして解決。

### OOM問題 (修正済み)

Qwen2.5のmax_position_embeddings=131072によるバッファ確保でOOMが発生。
MAX_INFERENCE_SEQ_LEN=4096を導入してバッファサイズを制限。
