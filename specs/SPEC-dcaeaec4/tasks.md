# SPEC-dcaeaec4 タスク一覧

## 概要

LLM-Router独自モデルストレージの実装タスク。

## 実装済み

- [x] FR-1: モデルディレクトリ構造 (`~/.llm-router/models/`)
- [x] FR-2: モデル名の形式変換 (`ModelStorage::modelNameToDir()`)
- [x] FR-3: モデルアーティファクト解決（safetensors/GGUF）を実装
- [x] FR-4: 利用可能モデル一覧をsafetensors対応
- [x] FR-5: `metadata.json` 依存を削除（読み書きしない）
- [x] FR-6: ノード起動時同期をsafetensors対応（必要ファイルを取得/参照）
  - [x] 起動時の不要モデル削除 (`ModelStorage::deleteModel()`)
- [x] FR-7: ルーターからのプッシュ通知 (`POST /api/models/pull`)
- [x] FR-8: API設計 (`/v0/models` vs `/v1/models`)

## テスト実装

- [x] `ModelStorage::deleteModel()` ユニットテスト
  - ファイル: `node/tests/unit/model_storage_test.cpp`
  - `DeleteModelRemovesDirectory` - ディレクトリ削除の検証
  - `DeleteNonexistentModelReturnsTrue` - 冪等性の検証

- [x] safetensorsアーティファクト解決のユニットテスト
  - ファイル: `node/tests/unit/model_storage_test.cpp`
  - `ResolveDescriptorFindsSafetensorsIndex` - index を優先すること

- [x] `is_ready()` チェック 統合テスト
  - ファイル: `node/tests/integration/openai_endpoints_test.cpp`
  - `Returns503WhenNotReady` - 同期中の503返却検証

- [x] 既存テストの修正
  - `openai_endpoints_test.cpp` - `set_ready(true)` 追加
  - `openai_api_test.cpp` (contract) - `SetUp()` で `set_ready(true)` 追加

## 実装ファイル

| ファイル | 変更内容 |
|---------|---------|
| `node/src/main.cpp` | FR-6/FR-7: 起動時同期＆プッシュ通知エンドポイント |
| `node/src/models/model_storage.cpp` | FR-1~5: モデルストレージ実装 |
| `node/src/api/openai_endpoints.cpp` | 同期中503返却 (`checkReady()`) |

## 検証済み動作

1. ノード起動時にルーターの `/v0/models` と同期
2. ルーターに存在しないモデルは自動削除
3. `POST /api/models/pull` でルーターからの通知を受信
4. 同期中は `/v1/chat/completions` 等が503を返却
5. 同期完了後は正常にリクエストを処理
