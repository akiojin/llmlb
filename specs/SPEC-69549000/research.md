# Research: safetensors.cpp

**機能ID**: SPEC-69549000
**日付**: 2026-01-06

## 調査結果サマリ

インタビューを通じて以下の技術選択が確定した。

## 1. safetensorsローダー

### 決定

stable-diffusion.cppの実装（`model.cpp`内の`init_from_safetensors_file`）を移植する。

### 理由

- 動作実績がある（stable-diffusion.cppで実運用中）
- 既にコードベース内に存在する（`node/third_party/stable-diffusion.cpp/`）
- MITライセンス互換

### 検討した代替案

| ライブラリ | URL | ライセンス | 評価 |
|-----------|-----|-----------|------|
| syoyo/safetensors-cpp | github.com/syoyo/safetensors-cpp | MIT | ヘッダーオンリー、最小依存だが機能限定 |
| hsnyder/safetensors.h | github.com/hsnyder/safetensors.h | Unlicense | 単一ヘッダー、シンプルだが機能限定 |
| sd.cpp実装 | node/third_party/stable-diffusion.cpp/model.cpp | MIT | **採用** - 実績あり |

### 実装参照

```cpp
// stable-diffusion.cpp/model.cpp より
bool ModelLoader::init_from_safetensors_file(const std::string& file_path, const std::string& prefix)
```

## 2. ggmlバージョン管理

### 決定

ggml本家リポジトリ（<https://github.com/ggml-org/ggml>）のmainブランチに追従する。

### 理由

- GPUバックエンド（Metal/CUDA/ROCm/Vulkan）がフル対応
- llama.cppへの依存を避けられる
- 独立プロジェクトとしての一貫性

### 検討した代替案

- llama.cpp内蔵ggml: llama.cppへの依存が発生、独立性が損なわれる

## 3. KVキャッシュ実装

### 決定

llama.cppのKVキャッシュ実装を参考に実装する。

### 理由

- 成熟した実装
- INT8/FP8量子化対応
- continuous batching対応

### 主要機能

- KVキャッシュの動的管理
- INT8/FP8量子化サポート
- プロンプトキャッシュ（再利用）

## 4. Attention実装

### 決定

ggml標準のAttention実装のみを使用する（Flash Attentionは当面不要）。

### 理由

- ggml標準で十分な性能
- 複雑性を避ける
- 将来的にggmlがFlash Attention対応すれば自動的に恩恵を受ける

## 5. マルチGPU戦略

### 決定

Pipeline Parallelism（レイヤー単位でGPU分割）を採用する。

### 理由

- 実装が比較的シンプル
- レイヤー単位の分割は理解しやすい
- ggmlのマルチデバイス対応と親和性が高い

### 検討した代替案

- Tensor Parallelism: 将来検討（実装複雑、通信オーバーヘッド大）

## 6. トークナイザー

### 決定

HuggingFaceの`tokenizer.json`を直接解析する。

### 理由

- HuggingFaceモデルとの互換性
- 外部依存を最小化
- llama.cppのトークナイザー実装を参考にできる

## 7. チャットテンプレート

### 決定

`tokenizer_config.json`の`chat_template`（Jinja2形式）を解析・適用する。

### 理由

- HuggingFaceの標準形式
- モデルごとのテンプレートを正しく適用できる
- llama.cppのテンプレートエンジン実装を参考にできる

## 8. エラーハンドリング

### 決定

llama.cppのコールバック方式を採用する。

### 理由

- C APIとの親和性
- 非同期エラー通知に対応
- 詳細なエラー情報を提供可能

## 9. スレッドセーフティ

### 決定

全てのAPIを完全スレッドセーフで設計する。

### 理由

- サーバー環境での並行リクエスト処理
- 安全なマルチスレッド利用

## 10. LoRA/QLoRA

### 決定

LoRAアダプターのロードとホットリロード（動的切り替え）をサポートする。

### 理由

- ファインチューニングモデルの柔軟な切り替え
- 運用時のアダプター更新に対応

## 11. プロンプトキャッシュ

### 決定

システムプロンプトやRAGコンテキストのKVキャッシュ再利用をサポートする。

### 理由

- TTFTの短縮
- 繰り返しプロンプトの効率化
- RAG利用時の性能向上

## 12. Visionモデル

### 決定

将来対応として画像入力をサポート予定。

### 理由

- LLaVA、Qwen-VL等のマルチモーダルモデル対応
- 画像エンコーダーはプラグイン拡張で対応

## 追加調査事項

### ggml safetensorsローダー

ggml本体にはsafetensorsローダーが含まれていない。
stable-diffusion.cppの実装を参考に、safetensors.cpp独自のローダーを実装する。

### テストモデル

HuggingFaceのTinyモデル（`hf-internal-testing/tiny-random-*`）を使用してCIテストを実行する。

### ベンチマーク

HuggingFace transformers（Python）との比較で性能を評価する。
主要指標:

- tokens/sec
- TTFT (Time To First Token)
- VRAM使用量（ピーク/平均）

## 13. Nemotron 3アーキテクチャ (Phase 9)

### 決定

Nemotron 3 Nanoのハイブリッド Mamba-Transformer MoE アーキテクチャを実装する。

### 理由

- Nemotron 3はLlamaベースではない独自アーキテクチャ
- Mamba-2を使用することでKVキャッシュを削減し、長コンテキスト（1M-token）に対応
- MoEで効率的なパラメータ活用を実現
- 2025年12月リリースの最新アーキテクチャ

### アーキテクチャ詳細

**モデル仕様**:
- 3.2B active parameters (31.6B total parameters)
- 52 layers total:
  - 23 MoE layers
  - 23 Mamba-2 layers
  - 6 GQA (Grouped Query Attention) layers with 2 groups
- 1M-token native context window

**MoE (Mixture of Experts)**:
- 128 routed experts per MoE layer
- 1 shared expert per MoE layer
- 6 experts activated per token (Top-6 routing)
- Load balancing mechanism

**Mamba-2 State Space Model**:
- Constant state storage during generation (vs linear KV cache)
- Replaces expensive self-attention layers
- Efficient long-context processing

**ハイブリッド構成**:
- MoE layers interleaved with Mamba-2 layers (predominantly)
- 6 GQA layers for critical attention needs
- Residual connections throughout

### 実装参考

- NVIDIA Technical Report: "Nemotron 3 Nano: Open, Efficient Mixture-of-Experts Hybrid Mamba-Transformer Model for Agentic Reasoning" (December 2025)
- HuggingFace model: `nvidia/NVIDIA-Nemotron-3-Nano-30B-A3B-BF16`
- Mamba公式実装: state-spaces/mamba
- MoE実装参考: Mixtral, DeepSeek-MoE

### 実装課題

1. **Mamba-2レイヤー**: ggmlにMamba-2カーネルが存在しない可能性 → カスタム実装が必要
2. **MoE routing**: Top-K選択とload balancingの実装
3. **ハイブリッド統合**: 3種類のレイヤー（MoE/Mamba-2/GQA）の統合
4. **1M-token対応**: メモリ効率的なstate管理

### 検討した代替案

- **Nemotron対応を見送る**: safetensors.cppを既存アーキテクチャ（Llama系）のみに限定
  - 却下理由: ユーザー要求により対応必須
- **Mamba-2を別ライブラリで実装**: state-spaces/mambaを統合
  - 検討中: ggmlとの統合性を考慮して決定

### MoE Routing実装詳細

**Load Balancer構成（Nemotron 3）**:

- **Load Balancer type**: 標準的なMLP router with sigmoid gating + squared ReLU activation
- **Expert選択**: 128 routed experts中、Top-6を選択
- **Shared experts**: 2個の共有エキスパート（全トークンで活性化）
- **Gating function**: Learnt multi-layer perceptron (MLP)

**Load Balancing戦略**:

- **Pre-training**: DeepSeek's aux-loss-free load balancing strategy（update rate 10^-3）+ standard load balancing loss
- **RL training**: MoE router weightsを凍結、expert biasのみ更新

**一般的なMoE Load Balancing課題と解決策**:

1. **課題**: Token-choice routingでは、一部の人気エキスパートに偏る（load imbalance）
2. **Auxiliary loss**: 全エキスパートに等しい重要性を与える制約を追加
3. **Random routing**: Top-2設定で、1番目は確定、2番目は重みに比例した確率で選択
4. **Expert capacity**: 各エキスパートが処理可能なトークン数の上限を設定
5. **Expert Choice Routing**: トークンがエキスパートを選ぶ代わりに、エキスパートがTop-kトークンを選ぶ（完全な負荷分散を保証）

**ggml/llama.cpp実装参考**:

- ggml: `GGML_OP_SSM_CONV`, `GGML_OP_SSM_SCAN`（Mamba SSM操作）
- llama.cpp: `src/models/mamba.cpp`（Mamba/Mamba2実装）
- CUDA backend: Mamba2サポート確認済み（[GitHub discussion #9196](https://github.com/ggml-org/llama.cpp/discussions/9196)）
