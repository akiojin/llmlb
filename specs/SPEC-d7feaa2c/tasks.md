# SPEC-d7feaa2c: Tasks

## Setup
- [x] Nodeエンジン抽象化の設計メモ整理
- [x] safetensors-cpp ヘッダをNode配下へ配置

## Core
- [ ] ModelStorageの`metadata.json`依存を削除し、safetensors/GGUFを検出できるようにする
- [x] EngineインターフェースとEngineRegistryを追加
- [x] LlamaEngineを既存実装から切り出し
- [x] NemotronEngineを追加（mmap + validation）
- [x] InferenceEngineからエンジン選択に委譲

## Integration
- [x] /v1/models で未対応モデルを除外するロジック追加
- [ ] Router/Nodeともに`metadata.json`を生成・参照しない
- [ ] 登録時の選択（safetensors/GGUF）に基づき、Nodeが適切なEngineを選択できるようにする（`config.json`優先）

## Tests
- [ ] ModelStorage: safetensors/GGUF検出テスト
- [x] Engine選択テスト
- [x] NemotronEngineロードテスト

## Docs
- [ ] Nodeのモデル登録/選択仕様をREADMEに追記（metadata.jsonなし）

## Polish
- [x] ログとエラーメッセージを整理
