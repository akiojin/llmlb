# SPEC-26006000: Plan

**仕様**: [spec.md](./spec.md)

## 目的
ASRは **safetensors正本**（whisper.cppでGGUF運用）、
TTSは **ONNX Runtime** で提供し、Node実行時のPython依存を排除する。

## 方針
- safetensorsを正本とし、safetensors/GGUF共存時は登録時にformat指定必須
- ASRはwhisper.cpp（GGML/GGUF運用）、TTSはONNX Runtime
- Node実行時はPython依存なし
- Whisper公式 `.pt` はPythonでsafetensors化し、正本として保持
- GPU前提（macOS: Apple Silicon / Linux&Windows: CUDA）

## 対象モデルとアーティファクト（前提）
- ASR: `openai/whisper-*` を含む音声認識モデル
  - `config.json` / `tokenizer.json` 必須
  - `*.safetensors`（シャーディングの場合は `.safetensors.index.json` 必須）
- TTS: ONNX配布の音声合成モデル
  - `*.onnx` を正本として扱う

## 役割分離
- Load Balancer: 登録/配布、必須メタデータ検証、manifest確定
- Node: whisper.cpp / ONNX Runtime で推論

## テスト方針（TDD）
- Contract → Integration → E2E → Unit → Core の順で実施
- 実GPU環境でのE2Eが必須

## 要明確化
- ASR/TTS の最初の対応モデル範囲（最小構成）
- safetensors 変換パイプラインの運用責任（Load Balancer/Node/外部運用）
