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

