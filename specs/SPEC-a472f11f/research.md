# Phase 0 リサーチ: 包括的E2Eテストスイート強化

**機能ID**: `SPEC-a472f11f` | **日付**: 2026-02-13

## 既存E2Eテスト基盤

### 現在のテスト構成 (68テスト: 61パス / 7スキップ)

| カテゴリ | ファイル | テスト数 |
|---------|---------|---------|
| auth | login.spec.ts | 7 |
| auth | register.spec.ts | 8 |
| auth | invitation.spec.ts | 6 |
| dashboard | dashboard-header.spec.ts | 7 |
| dashboard | dashboard-nodes.spec.ts | 12 (7 skipped) |
| dashboard | dashboard-stats.spec.ts | 8 |
| dashboard | endpoint-status-colors.spec.ts | 1 |
| api | error-handling.spec.ts | 8 |
| workflows | api-key-openai-e2e.spec.ts | 1 |
| workflows | endpoint-playground-walkthrough.spec.ts | 1 |
| workflows | lb-playground-walkthrough.spec.ts | 1 |
| workflows | model-registration.spec.ts | 5 |

### 既存ヘルパー

- **api-helpers.ts**: `ensureDashboardLogin`, `listEndpoints`, `deleteEndpointsByName`,
  `createTestModel`, `cleanTestState`, `listModels`, `listApiKeys`
- **mock-openai-endpoint.ts**: `/v1/models`, `/v1/chat/completions` (stream/non-stream), `/api/health`
- **selectors.ts**: `DashboardSelectors`, `PlaygroundSelectors`
- **mock-helpers.ts**: テストデータ生成

### 既存Page Object

- **auth.page.ts**: `LoginPage`, `RegisterPage`
- **dashboard.page.ts**: `DashboardPage` (エンドポイント一覧・追加・検索)
- **playground.page.ts**: Playground操作

## 対象RustバックエンドAPI調査

### ダッシュボードAPI

| エンドポイント | ハンドラー | 用途 |
|--------------|-----------|------|
| GET `/api/endpoints` | `api/endpoints.rs` | エンドポイント一覧 |
| POST `/api/endpoints` | `api/endpoints.rs` | エンドポイント作成 |
| PUT `/api/endpoints/{id}` | `api/endpoints.rs` | エンドポイント更新 (name, health_check_interval, inference_timeout, notes) |
| DELETE `/api/endpoints/{id}` | `api/endpoints.rs` | エンドポイント削除 |
| POST `/api/endpoints/{id}/test` | `api/endpoints.rs` | 接続テスト |
| POST `/api/endpoints/{id}/sync` | `api/endpoints.rs` | モデル同期 |
| POST `/api/endpoints/{id}/download` | `api/endpoints.rs` | モデルダウンロード (xLLMのみ) |
| GET `/api/users` | `api/users.rs` | ユーザー一覧 |
| POST `/api/users` | `api/users.rs` | ユーザー作成 |
| PUT `/api/users/{id}` | `api/users.rs` | ユーザー更新 |
| DELETE `/api/users/{id}` | `api/users.rs` | ユーザー削除 |
| GET `/api/dashboard/logs/lb` | `api/logs.rs` | LBログ取得 |
| GET `/api/nodes/{id}/logs` | `api/logs.rs` | エンドポイント別ログ |
| GET `/api/metrics/cloud` | `cloud_metrics.rs` | Prometheusメトリクス |
| GET `/api/system` | `api/system.rs` | システム情報 |
| POST `/api/system/update/apply` | `api/system.rs` | システム更新適用 |

### OpenAI互換API

| エンドポイント | ハンドラー | 用途 |
|--------------|-----------|------|
| POST `/v1/chat/completions` | `api/chat.rs` | チャット補完 (stream対応) |
| GET `/v1/models` | `api/models.rs` | モデル一覧 |
| POST `/v1/audio/transcriptions` | `api/audio.rs` | 音声文字起こし |
| POST `/v1/audio/speech` | `api/audio.rs` | 音声合成 |
| POST `/v1/images/generations` | `api/images.rs` | 画像生成 |
| POST `/v1/images/edits` | `api/images.rs` | 画像編集 |
| POST `/v1/images/variations` | `api/images.rs` | 画像バリエーション |
| POST `/v1/responses` | `api/responses.rs` | レスポンスAPI |

### 権限システム (11種類)

| 権限名 | 対応スコープ |
|--------|-------------|
| OpenaiInference | `/v1/chat/completions`, `/v1/audio/*`, `/v1/images/*`, `/v1/responses` |
| OpenaiModelsRead | `/v1/models` |
| EndpointsRead | GET `/api/endpoints` |
| EndpointsManage | POST/PUT/DELETE `/api/endpoints` |
| ApiKeysManage | `/api/keys` |
| UsersManage | `/api/users` |
| InvitationsManage | `/api/invitations` |
| ModelsManage | `/api/models` |
| RegistryRead | `/api/registry` |
| LogsRead | `/api/dashboard/logs/*`, `/api/nodes/*/logs` |
| MetricsRead | `/api/metrics/cloud` |

### エンドポイントタイプ検出ロジック

`detection/mod.rs`の検出順序:

1. **xLLM**: `/v0/system`応答でxLLM固有フィールド検出
2. **Ollama**: `/api/tags`応答でOllama固有フィールド検出
3. **vLLM**: `/v1/models`応答でvLLM固有フィールド検出 (vllm metadata)
4. **OpenAI**: `/v1/models`で標準OpenAI応答
5. **Unknown**: 上記いずれにも該当しない

### ダッシュボードUIコンポーネント

| コンポーネント | ファイル | テスト状況 |
|--------------|---------|----------|
| TokenStatsSection | `components/dashboard/TokenStatsSection.tsx` | テストゼロ |
| RequestHistoryTable | `components/dashboard/RequestHistoryTable.tsx` | テストゼロ |
| LogViewer | `components/dashboard/LogViewer.tsx` | テストゼロ |
| UserModal | `components/users/UserModal.tsx` | テストゼロ |
| EndpointDetailModal | `components/dashboard/EndpointDetailModal.tsx` | 部分的 |
| PlaygroundSettings | 各Playgroundコンポーネント | テストゼロ |
| SystemUpdateBanner | `components/dashboard/SystemUpdateBanner.tsx` | テストゼロ |

## モックサーバー拡張方針

### mock-openai-endpoint.ts への追加

既存のモックサーバーに以下のハンドラーを追加:

1. **Audio API**: `/v1/audio/transcriptions` (multipart), `/v1/audio/speech` (binary response)
2. **Image API**: `/v1/images/generations`, `/v1/images/edits` (multipart), `/v1/images/variations` (multipart)
3. **Responses API**: `/v1/responses`
4. **エンドポイントタイプ応答**: `/v0/system` (xLLM), `/api/tags` (Ollama) のオプション応答
5. **ダウンロードAPI**: `/v0/models/download` (xLLMモック)

### 権限マトリクステスト設計

11権限 × 主要エンドポイント = 約60-80テストケース。
テストはパラメトリックに生成し、`test.describe`で権限ごとにグループ化。

## リスク・制約

| リスク | 緩和策 |
|--------|--------|
| テスト実行時間増大 | 並列実行可能なテストは`mode: 'parallel'`、依存テストのみ`mode: 'serial'` |
| モックサーバーの複雑化 | ハンドラーをルート単位で分離、共通ユーティリティを抽出 |
| 権限マトリクスの組み合わせ爆発 | パラメトリックテスト生成で重複コードを排除 |
| JWT期限切れテストの実装 | llmlbのデバッグモードでJWT有効期限を短縮する方法を調査（またはAPI直接テスト） |
