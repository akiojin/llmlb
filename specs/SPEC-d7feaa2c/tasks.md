# SPEC-d7feaa2c: Tasks

## Note
- Nemotron向けの新エンジン（推論エンジン）は後日仕様化（TBD）。本タスクはエンジンローダー抽象化が主対象。

## Setup
- [x] Nodeエンジン抽象化の設計メモ整理
- [x] safetensors-cpp ヘッダをNode配下へ配置

## Core
- [x] ModelStorageの`metadata.json`依存を削除し、safetensors/GGUFを検出できるようにする
- [x] EngineインターフェースとEngineRegistryを追加
- [x] LlamaEngineを既存実装から切り出し
- [x] InferenceEngineからエンジン選択に委譲

## Plugin Migration（追加）
- [x] EngineHost（プラグインローダー）を導入する
- [x] Engine ABI/manifest の必須項目・ABI一致を検証する
- [x] Engine ABI/manifest をJSONとして定義し、互換性検証を実装する
- [x] llama.cpp を plugin 化してロードできるようにする

## Integration
- [x] /v1/models で未対応モデルを除外するロジック追加
- [x] Router/Nodeともに`metadata.json`を生成・参照しない
- [x] 登録時の選択（safetensors/GGUF）に基づき、Nodeが適切なEngineを選択できるようにする（`config.json`優先）

## Tests
- [x] ModelStorage: safetensors/GGUF検出テスト
- [x] Engine選択テスト
- [x] EngineHost: プラグイン manifest 検証テスト

## Docs
- [x] Nodeのモデル登録/選択仕様をREADMEに追記（metadata.jsonなし）

## Polish
- [x] ログとエラーメッセージを整理

## Spike（任意）
- [x] NemotronEngineを追加（mmap + validation）
- [x] NemotronEngineロードテスト

## Deferred（TBD）
- Nemotron向けの新エンジン（推論エンジン）の仕様策定（別SPEC）
- Nemotron向けの新エンジン（推論エンジン）の実装（Metal/CUDA）
- Router側: HF chat_template(Jinja) を完全互換でレンダリングし、Nodeへ最終プロンプトを渡す方針の具体化（別SPEC想定）
- Nemotron GPU PoC: safetensors直読→GPU演算→E2E生成までの段階的検証（別SPEC想定）
