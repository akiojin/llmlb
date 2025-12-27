# SPEC-6cd7f960: タスク一覧

## Phase 1: 静的モデル定義とAPI（Router）

### Setup

- [x] `router/src/supported_models.rs` 新規作成
- [x] `router/src/lib.rs` に `mod supported_models` 追加

### Test (RED)

- [x] `supported_models::get_supported_models()` のテスト作成
- [x] `ModelStatus` enum のテスト作成
- [x] `ModelWithStatus` 構造体のテスト作成
- [x] `GET /v0/models/hub` のテスト作成
- [x] `POST /v0/models/pull` のテスト作成
- [x] 存在しないmodel_idでのエラーテスト作成

### Core

- [x] `SupportedModel` 構造体定義
- [x] `get_supported_models()` 実装（初期モデル5つ）
- [x] `ModelStatus` enum 定義
- [x] `ModelWithStatus` 構造体定義
- [x] `list_models_with_status()` 実装（対応モデル + 状態を返す）
- [x] `pull_model()` 実装（ConvertTaskManagerへのキュー登録）
- [x] HF動的情報取得実装（キャッシュ付き）

### Integration

- [x] `router/src/api/mod.rs` ルーティング追加（`/v0/models/hub`, `/v0/models/pull`）

## Phase 2: ダッシュボードUI

### Setup

- [x] `ModelHubTab.tsx` 新規作成
- [x] `ModelCard.tsx` 新規作成（ModelHubTab内に実装）

### Core

- [x] `api.ts` に型定義追加（`SupportedModel`, `ModelWithStatus`）
- [x] `api.ts` に `modelsApi.getHub()` 追加
- [x] `api.ts` に `modelsApi.pull()` 追加
- [x] `ModelsSection.tsx` タブ化（Local / Model Hub）
- [x] `ModelHubTab.tsx` 実装（カードグリッド、検索、Pullボタン）
- [x] `ModelCard.tsx` 実装（状態に応じた表示）

## Phase 3: 廃止機能削除

### Core

- [x] `router/src/api/models.rs` から `register_model()` 削除
- [x] `router/src/api/models.rs` から `discover_gguf_endpoint()` 削除
- [x] `router/src/api/models.rs` から関連構造体削除
- [x] `router/src/api/mod.rs` から `/models/register` ルート削除
- [x] `router/src/api/mod.rs` から `/models/discover-gguf` ルート削除

### Dashboard

- [x] `ModelsSection.tsx` から `RegisterDialog` 削除
- [x] `ModelsSection.tsx` から `registerMutation` 削除
- [x] `ModelsSection.tsx` から `Register` ボタン削除
- [x] `api.ts` から `modelsApi.register()` 削除

### Test

- [x] 関連テスト更新・削除

## Polish

- [x] `cargo fmt --check` 合格
- [x] `cargo clippy -- -D warnings` 合格
- [x] `cargo test` 合格
- [x] markdownlint 合格
- [ ] E2Eテスト確認
