# SPEC-d7feaa2c: Plan

## 方針
- Node側にエンジン抽象化レイヤーを導入し、実行エンジンを差し替え可能にする
- 既存 llama.cpp を Engine としてラップする
- エンジン選択は「登録時に選択したアーティファクト（safetensors/GGUF）」と
  Hugging Face の `config.json` 等のモデル由来メタデータを正として判定する
- `metadata.json` のような llm-router 独自メタデータファイルは使用しない
- Nemotron向けの新エンジン（推論エンジン）の仕様/実装は別SPECで後日決定（TBD）

## 実装概要

### 1) Node側抽象化
- `Engine` インターフェース
- `EngineRegistry` で runtime ID → engine 実装を解決
- `InferenceEngine` は内部で engine を選択して処理を委譲

### 2) ModelStorage拡張
- 登録時に選択されたアーティファクト（`format` / `filename` / `gguf_policy` 等）と
  `config.json` 等のメタデータを読み取り、`ModelDescriptor` として返す
- `listAvailable()` は「選択されたアーティファクトがローカルに存在するか」と
  「対応エンジンがあるか」で有効モデルを列挙する

### 3) Router側最小対応
- Nodeが必要とする「登録時の選択情報」を永続化し、Nodeへ渡せるようにする
- `metadata.json` を生成・参照しない

## テスト
- ModelStorage: `format` / 必須メタデータの検証テスト
- Engine selection: 登録時選択と `config.json` に基づく判定テスト
