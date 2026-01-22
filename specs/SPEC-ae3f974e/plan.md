# SPEC-ae3f974e: Plan

**仕様**: [spec.md](./spec.md)

## 目的
画像生成を **safetensors正本**で提供し、
アーティファクト選択は Node が実行環境に応じて行う。

## 方針
- Node実行時はPython依存なし
- safetensorsを正本とし、GGUF/Metalは Node が選択する
- GPU前提（macOS: Metal / Windows: DirectML / Linux: 実験）

## 対象モデルとアーティファクト（前提）
- 画像生成モデル（SD系を含む）
  - `config.json` / `tokenizer.json` 必須
  - `*.safetensors`（シャーディングの場合は `.safetensors.index.json` 必須）

## 役割分離
- Load Balancer: 登録/メタデータ保存、manifest提供
- Node: 画像生成エンジンでsafetensors直読（必要に応じてHFから取得）

## テスト方針（TDD）
- Contract → Integration → E2E → Unit → Core の順で実施

## 要明確化
- safetensors直読の画像生成エンジン選定
