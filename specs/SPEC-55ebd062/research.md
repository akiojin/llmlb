# Research: Nemotron CUDA PoC

**SPEC**: SPEC-55ebd062
**日付**: 2025-12-24

## 1. Nemotronアーキテクチャ

### 決定: Llama系アーキテクチャベース

**理由**: NemotronはLlamaアーキテクチャをベースとしており、以下の構成を持つ:

- **Embedding**: token_embeddings
- **Transformer Layers**: N個のデコーダーレイヤー
  - RMSNorm (input_layernorm)
  - Self-Attention (q_proj, k_proj, v_proj, o_proj)
  - RMSNorm (post_attention_layernorm)
  - MLP (gate_proj, up_proj, down_proj + SiLU activation)
- **Output**: RMSNorm + lm_head

**検討した代替案**:

- GPT-2スタイル: LayerNormの位置が異なる
- Mistral: Sliding Window Attentionが追加

### config.jsonパラメータ

```json
{
  "hidden_size": 4096,
  "intermediate_size": 14336,
  "num_attention_heads": 32,
  "num_hidden_layers": 32,
  "num_key_value_heads": 8,
  "vocab_size": 256000,
  "rms_norm_eps": 1e-5,
  "rope_theta": 10000.0
}
```

## 2. CUDA演算パターン

### 決定: cuBLAS GEMM + カスタムカーネル

**理由**:

- **GEMM (行列積)**: cuBLAS `cublasGemmEx` を使用（BF16/FP16対応）
- **Attention**: カスタムCUDAカーネル（Flash Attention簡易版）
- **Activation (SiLU)**: カスタムCUDAカーネル
- **RMSNorm**: カスタムCUDAカーネル
- **Softmax**: カスタムCUDAカーネル

**検討した代替案**:

- cuDNN: オーバーヘッドが大きい（PoCには過剰）
- Triton: Python依存（C++のみ方針に反する）
- FlashAttention公式: 複雑すぎる（PoCには過剰）

### メモリレイアウト

```text
BF16テンソル: Row-major (safetensorsデフォルト)
CUDA: Row-major維持（転置不要）
cuBLAS: CUBLAS_OP_N / CUBLAS_OP_T で対応
```

## 3. トークナイザー処理

### 決定: 簡易BPEデコーダー自前実装

**理由**:

- tokenizer.jsonからvocab + mergesを読み込み
- エンコード: 文字列 → BPEトークン列
- デコード: トークンID → 文字列
- 外部ライブラリ依存を避ける（PoCのシンプルさ優先）

**検討した代替案**:

- SentencePiece C++: 追加依存関係が増える
- llama.cpp tokenizer: llama.cpp依存を避ける方針に反する
- HuggingFace tokenizers (Rust): FFI複雑

### 実装範囲

- ASCII + 基本UTF-8のみサポート（PoC範囲）
- 特殊トークン（BOS, EOS）対応
- 複雑なnormalizationはスキップ

## 4. 既存実装の活用

### 決定: poc/nemotron-safetensors-cpp を基盤として拡張

**理由**:

- safetensors.hh（ヘッダーオンリー）を流用
- mmap_from_file()でメモリマップ済み
- tensor_t構造体でテンソル情報取得済み

**活用ポイント**:

```cpp
// 既存: safetensors読み込み
safetensors::mmap_from_file(filename, &st, &warn, &err);
safetensors::tensor_t tensor;
st.tensors.at(name, &tensor);

// 追加: CUDAメモリへ転送
cudaMalloc(&d_tensor, tensor.data_bytes);
cudaMemcpy(d_tensor, tensor.data, tensor.data_bytes, cudaMemcpyHostToDevice);
```

### node/engines/NemotronEngineからの学び

- validate_nemotron_format(): メタデータ検証ロジック
- 必須ファイル: config.json, tokenizer.json, *.safetensors

## 5. ビルドシステム

### 決定: CMake + CUDA

**理由**:

- node/CMakeLists.txtと同様のパターン
- find_package(CUDAToolkit)
- enable_language(CUDA)

**CMakeLists.txt概要**:

```cmake
cmake_minimum_required(VERSION 3.18)
project(nemotron-cuda-poc LANGUAGES CXX CUDA)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CUDA_STANDARD 17)

find_package(CUDAToolkit REQUIRED)

add_executable(nemotron-cuda-poc
    src/main.cpp
    src/safetensors_loader.cpp
    src/cuda_memory.cu
    src/transformer.cu
    src/tokenizer.cpp
    src/inference.cpp
)

target_link_libraries(nemotron-cuda-poc
    CUDA::cudart
    CUDA::cublas
)
```

## 6. エラーハンドリング

### 決定: 即時終了 + 明確なエラーメッセージ

**理由**: PoCのため、リカバリー不要。原因特定を優先。

**エラーカテゴリ**:

1. **ファイルエラー**: モデルパス不正、必須ファイル欠損
2. **CUDAエラー**: デバイス未検出、メモリ不足、カーネルエラー
3. **モデルエラー**: config不正、テンソル形状不一致

**エラー出力形式**:

```text
[ERROR] Category: Message
        Details: 詳細情報
        Hint: 解決策のヒント
```

## まとめ

| 項目 | 決定 |
|------|------|
| アーキテクチャ | Llama系（RMSNorm + SiLU + RoPE） |
| CUDA演算 | cuBLAS GEMM + カスタムカーネル |
| トークナイザー | 簡易BPE自前実装 |
| 基盤コード | poc/nemotron-safetensors-cpp |
| ビルド | CMake + CUDA |
| エラー処理 | 即時終了 + 明確なメッセージ |
