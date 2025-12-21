# SPEC-d7feaa2c: Plan

## 方針
- Node側にエンジン抽象化レイヤーを導入
- 既存 llama.cpp を LlamaEngine としてラップ
- NemotronEngine を追加し、safetensors-cpp で直接ロード
- metadata.json をエンジン選択の一次情報源にする（後方互換は fallback）

## 実装概要

### 1) メタデータ仕様
- `models/<model_id>/metadata.json` を参照
- 例:
  ```json
  {
    "runtime": "llama_cpp",
    "format": "gguf",
    "primary": "model.gguf"
  }
  ```
  ```json
  {
    "runtime": "nemotron_cpp",
    "format": "safetensors",
    "primary": "model.safetensors.index.json"
  }
  ```

### 2) Node側抽象化
- `Engine` インターフェース
- `EngineRegistry` で runtime ID → engine 実装を解決
- `InferenceEngine` は内部で engine を選択して処理を委譲

### 3) NemotronEngine
- safetensors-cpp を Node に組み込み
- mmapで読み込み → validate → テンソル検査
- 主要テンソル存在の検証（experts関連）

### 4) ModelStorage拡張
- metadataを解析して `ModelDescriptor` を返す
- `listAvailable()` は metadata優先で有効モデルを列挙

### 5) Router側最小対応
- GGUFモデルに metadata.json を生成（runtime/format/primary）
- 既存挙動は維持

## テスト
- ModelStorage metadata 解析ユニットテスト
- Engine selection のユニットテスト
- NemotronEngine のロード検証テスト（小さなサンプルsafetensors）

