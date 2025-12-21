# SPEC-d7feaa2c: Tasks

## Setup
- [x] Nodeエンジン抽象化の設計メモ整理
- [x] safetensors-cpp ヘッダをNode配下へ配置

## Core
- [x] ModelStorageにmetadata読み取りとModelDescriptorを追加
- [x] EngineインターフェースとEngineRegistryを追加
- [x] LlamaEngineを既存実装から切り出し
- [x] NemotronEngineを追加（mmap + validation）
- [x] InferenceEngineからエンジン選択に委譲

## Integration
- [x] /v1/models で未対応モデルを除外するロジック追加
- [x] Router側でGGUF向けmetadata.json生成

## Tests
- [x] ModelStorage metadata解析テスト
- [x] Engine選択テスト
- [x] NemotronEngineロードテスト

## Docs
- [x] Nodeのmetadata仕様をREADMEに追記（簡潔に）

## Polish
- [x] ログとエラーメッセージを整理
