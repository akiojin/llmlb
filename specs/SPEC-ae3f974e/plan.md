# SPEC-ae3f974e: Plan

**仕様**: [spec.md](./spec.md)

## 目的
画像生成を **safetensors正本**で提供し、safetensors/GGUF共存時は登録時のformat指定を必須とする。

## 方針
- Node実行時はPython依存なし
- safetensorsを正本とし、GGUFは登録時に `format=gguf` を選択した場合のみ使用
- GPU前提（macOS: Apple Silicon / Linux&Windows: CUDA）

## 対象モデルとアーティファクト（前提）
- 画像生成モデル（SD系を含む）
  - `config.json` / `tokenizer.json` 必須
  - `*.safetensors`（シャーディングの場合は `.safetensors.index.json` 必須）

## 役割分離
- Router: 登録/配布、必須メタデータ検証、manifest確定
- Node: 画像生成エンジンでsafetensors直読

## テスト方針（TDD）
- Contract → Integration → E2E → Unit → Core の順で実施

## 要明確化
- safetensors直読の画像生成エンジン選定
- GGUF選択時のUI/運用条件
