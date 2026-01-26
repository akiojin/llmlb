# SPEC-dcaeaec4 タスク一覧

## 概要

LLM-Load Balancer独自モデルストレージの実装タスク。

## 実装済み

- [x] FR-1: モデルディレクトリ構造 (`~/.llmlb/models/`)
- [x] FR-2: モデル名の形式変換 (`ModelStorage::modelNameToDir()`)
- [x] FR-3: モデルアーティファクト解決（Node主導/外部ソース対応）を実装
- [x] FR-4: 利用可能モデル一覧をsafetensors対応
- [x] FR-5: `metadata.json` 依存を削除（読み書きしない）
- [x] FR-6: ノード起動時同期（マニフェスト+外部ソース/プロキシ）を実装
  - [x] 起動時の不要モデル削除 (`ModelStorage::deleteModel()`)
- [x] FR-7: ロードバランサーからのプッシュ通知 (`POST /api/models/pull`)
- [x] FR-8: API設計更新（`/v0/models/registry` 追加と Node 同期の整理）

## 追加対応（要件更新）

- [x] Node: GPUバックエンドに応じたアーティファクト選択（Metal/DirectML）
- [x] Load Balancer: マニフェストに外部URL/許可リスト情報を含める
- [x] Node: 外部ソース/プロキシ経由ダウンロード（許可リスト検証付き）

## 追加対応（Session 2025-12-31）

- [x] Load Balancer: モデル登録時のバイナリダウンロード/変換を廃止（メタデータのみ保存）
- [x] Load Balancer: マニフェスト生成をローカルファイル依存からHFメタデータ基準へ変更（URLのみ提示）
- [x] Load Balancer: `/v0/models/registry/:model_name/files/:file` の必須依存を削除（必要ならリモートストリーミングに限定）
- [x] Node: マニフェストのURLから**直接HF取得**するフローに統一（プロキシは非必須）
- [x] Node: HFトークンをNode環境変数で扱う方針へ更新（ロードバランサー経由で渡さない）
- [x] Tests: ロードバランサーキャッシュ前提のテストを削除/更新

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

1. ノード起動時にロードバランサーの `/v0/models` と同期
2. ロードバランサーに存在しないモデルは自動削除
3. `POST /api/models/pull` でロードバランサーからの通知を受信
4. 同期中は `/v1/chat/completions` 等が503を返却
5. 同期完了後は正常にリクエストを処理
