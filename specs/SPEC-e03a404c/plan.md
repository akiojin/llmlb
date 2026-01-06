# SPEC-e03a404c: Plan

**仕様**: [spec.md](./spec.md)

## 目的
画像認識（Vision）を **safetensors正本**で提供し、GGUFはsafetensors不在時のみフォールバックする。

## 方針
- Node実行時はPython依存なし
- safetensorsを正本とし、GGUFは限定的フォールバック
- GPU前提（macOS: Apple Silicon / Linux&Windows: CUDA）

## 対象モデルとアーティファクト（前提）
- Vision対応モデル
  - `config.json` / `tokenizer.json` 必須
  - `*.safetensors`（シャーディングの場合は `.safetensors.index.json` 必須）

## 役割分離
- Router: 画像付きchatの検証・パース、必須メタデータ検証、manifest確定
- Node: 画像認識エンジンでsafetensors直読

## テスト方針（TDD）
- Contract → Integration → E2E → Unit → Core の順で実施

## 要明確化
- Vision向け新エンジンの選定
- GGUFフォールバック時の許可条件（登録時/実行時）
