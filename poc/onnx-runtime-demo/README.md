# ONNX Runtime PoC (macOS / M4 想定)

目的: 既存の埋め込み型アーキテクチャを崩さず、ONNX Runtime (CoreML EP) を node プロセス内で動かせるかを検証する最小サンプルです。モデルの実行はせず、セッション生成と入出力メタデータの取得まで行います。

## 必要環境
- macOS (Apple Silicon、M4 想定)
- CMake 3.20+
- ONNX Runtime (CoreML EP を含む arm64 ビルド)
  - Homebrew 例: `brew install onnxruntime`
  - もしくは pip wheel: `pip install onnxruntime`（C++ 連携時はヘッダとライブラリのパスを自前で通す必要があります）

## ビルド
```bash
cd poc/onnx-runtime-demo
cmake -B build -DCMAKE_BUILD_TYPE=Release
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
Execution providers (priority order):
  - CoreMLExecutionProvider
  - CPUExecutionProvider
Inputs: 1
  [0] input shape=(1, 3, 224, 224)
Session initialization OK.
```

## HFモデルの直接変換（例）
`convert_and_run.sh` で PyTorch→ONNX 変換と PoC 実行を一括実行できます。

```bash
./convert_and_run.sh                             # tiny BERT を変換して PoC 実行
MODEL=microsoft/VibeVoice-Realtime-0.5B ./convert_and_run.sh
```

### 既知の制約
- Homebrew の onnxruntime ボトルは CoreML/XNNPACK EP 非同梱のため CPU 実行のみ。M4 の GPU/ANE を使うには `--use_coreml` 付きで onnxruntime をソースビルドする必要があります。
- `microsoft/VibeVoice-Realtime-0.5B` は Transformers の標準エクスポーター（sequence-classification 等）に未対応のカスタムアーキテクチャです。`transformers.onnx` では変換できず、独自のエクスポートスクリプトが必要です（音響トークナイザ＋拡散ヘッドを含むため）。

## メモ
- CoreML EP が見つからない場合は CPU にフォールバックします。
- 実際の推論ループや I/O 前処理は入れていません。必要になったらこのサンプルをベースに拡張してください。
