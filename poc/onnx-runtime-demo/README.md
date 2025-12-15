# ONNX Runtime PoC (macOS / M4 想定)

目的: 既存の埋め込み型アーキテクチャを崩さず、ONNX Runtime (CoreML EP) を node プロセス内で動かせるかを検証する最小サンプルです。モデルの実行はせず、セッション生成と入出力メタデータの取得まで行います。

## 必要環境
- macOS (Apple Silicon、M4 想定)
- CMake 3.20+
- ONNX Runtime (arm64)
  - Homebrew: `brew install onnxruntime`（※通常は **CPU EP のみ**。CoreML/XNNPACK は未同梱のことが多い）
  - CoreML EP を使う場合: ソースビルドして CMake package を用意する（下記参照）

## ビルド
```bash
cmake -S poc/onnx-runtime-demo -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j
```

CoreML EP 付きの onnxruntime を自前ビルドした場合は `onnxruntime_DIR` を指定します:
```bash
cmake -S poc/onnx-runtime-demo -B build -DCMAKE_BUILD_TYPE=Release \
  -Donnxruntime_DIR=/path/to/onnxruntime/install/lib/cmake/onnxruntime
cmake --build build -j
```

## 実行
```bash
./build/onnx_poc /path/to/model.onnx
```

出力例:
```
CoreML EP enabled
Loaded model: /path/to/model.onnx
Available providers:
  - CoreMLExecutionProvider
  - CPUExecutionProvider
Inputs: 1
  [0] input shape=(1, 3, 224, 224)
Session initialization OK.
```

## CoreML EP 付き onnxruntime のビルド（macOS）
このリポジトリの `scripts/build-onnxruntime-coreml.sh` を使うと、CoreML EP 有効の onnxruntime を
ビルドして `find_package(onnxruntime)` で参照できる形（CMake package）までインストールします。

```bash
./scripts/build-onnxruntime-coreml.sh
```

## HFモデルの直接変換（例）
`convert_and_run.sh` で PyTorch→ONNX 変換と PoC 実行を一括実行できます。

```bash
./convert_and_run.sh                             # tiny BERT を変換して PoC 実行
MODEL=microsoft/VibeVoice-Realtime-0.5B ./convert_and_run.sh
```

### 既知の制約
- Homebrew の onnxruntime ボトルは CoreML EP 非同梱のことが多く、この PoC は **エラー終了** します（CPUフォールバック無し）。M4 の GPU/ANE を使うには `--use_coreml` 付きで onnxruntime をソースビルドする必要があります。
- `microsoft/VibeVoice-Realtime-0.5B` は Transformers の標準エクスポーター（sequence-classification 等）に未対応のカスタムアーキテクチャです。`transformers.onnx` では変換できず、独自のエクスポートスクリプトが必要です（音響トークナイザ＋拡散ヘッドを含むため）。

## メモ
- CoreML EP が見つからない場合は CPU にフォールバックします。
- 実際の推論ループや I/O 前処理は入れていません。必要になったらこのサンプルをベースに拡張してください。
