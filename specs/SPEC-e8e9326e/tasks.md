# タスク: ロードバランサー主導エンドポイント登録システム

**機能ID**: `SPEC-e8e9326e`
**ステータス**: 部分完了（Phase 4 実装済み、テスト/ハーネス未復帰あり）
**入力**: `/specs/SPEC-e8e9326e/` の設計ドキュメント
**前提条件**: plan.md, research.md, data-model.md, contracts/endpoints-api.md, quickstart.md

## 実行フロー

```text
1. データベースマイグレーション追加
2. 型定義・モデル作成
3. Contract Tests作成（RED）→ 実装（GREEN）
4. Integration Tests作成（RED）→ 実装（GREEN）
5. ヘルスチェッカー実装
6. ルーティング統合
7. 旧コード削除
8. ドキュメント更新
```

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能（異なるファイル、依存関係なし）

## Phase 3.1: セットアップ

- [x] T001 `llmlb/migrations/` に `YYYYMMDDHHMMSS_add_endpoints.sql` マイグレーション追加（endpoints, endpoint_models, endpoint_health_checks）
- [x] T002 [P] `llmlb/src/types/endpoint.rs` に型定義を作成（Endpoint, EndpointStatus, EndpointModel, EndpointHealthCheck）
- [x] T003 [P] `llmlb/src/db/mod.rs` に `endpoints` モジュールを追加
- [x] T004 [P] `llmlb/src/api/mod.rs` に `endpoints` モジュールを追加

## Phase 3.2: テストファースト (TDD) ⚠️ 3.3の前に完了必須

**重要: これらのテストは記述され、実装前に失敗する必要がある**

### Contract Tests（API契約検証）✅ RED完了

- [x] T005 [P] `llmlb/tests/contract/endpoints_post_test.rs` に POST /api/endpoints の contract test
- [x] T006 [P] `llmlb/tests/contract/endpoints_get_list_test.rs` に GET /api/endpoints の contract test
- [x] T007 [P] `llmlb/tests/contract/endpoints_get_detail_test.rs` に GET /api/endpoints/:id の contract test
- [x] T008 [P] `llmlb/tests/contract/endpoints_put_test.rs` に PUT /api/endpoints/:id の contract test
- [x] T009 [P] `llmlb/tests/contract/endpoints_delete_test.rs` に DELETE /api/endpoints/:id の contract test
- [x] T010 [P] `llmlb/tests/contract/endpoints_test_test.rs` に POST /api/endpoints/:id/test の contract test
- [x] T011 [P] `llmlb/tests/contract/endpoints_sync_test.rs` に POST /api/endpoints/:id/sync の contract test

### Integration Tests（ユーザーストーリー検証）✅ RED完了

- [x] T012 [P] `llmlb/tests/integration/endpoint_registration_test.rs` にUS1: エンドポイント登録のintegration test
- [x] T013 [P] `llmlb/tests/integration/endpoint_health_check_test.rs` にUS2: 稼働状況監視のintegration test
- [x] T014 [P] `llmlb/tests/integration/endpoint_model_sync_test.rs` にUS3: モデル同期のintegration test
- [x] T015 [P] `llmlb/tests/integration/endpoint_connection_test_test.rs` にUS4: 接続テストのintegration test
- [x] T016 [P] `llmlb/tests/integration/endpoint_management_test.rs` にUS5: 管理操作のintegration test
- [x] T016a [P] `llmlb/tests/integration/endpoint_name_uniqueness_test.rs` に名前重複検証のintegration test
- [x] T016b [P] `llmlb/tests/integration/endpoint_latency_routing_test.rs` にレイテンシベースルーティングのintegration test
- [x] T016c [P] `llmlb/tests/integration/endpoint_auto_recovery_test.rs` に自動復旧のintegration test
- [x] T016d [P] `llmlb/tests/integration/endpoint_viewer_access_test.rs` にviewerロール制限のintegration test

## Phase 3.3: コア実装（テストが失敗した後のみ） ✅ GREEN完了

### DB層

- [x] T017 `llmlb/src/db/endpoints.rs` にEndpointStorage CRUD実装（name UNIQUE制約、latency_ms含む）
- [x] T018 `llmlb/src/db/endpoints.rs` にEndpointModelStorage CRUD実装（endpoints.rsに統合）
- [x] T018a `llmlb/src/db/endpoints.rs` にEndpointHealthCheckStorage実装（履歴保存・30日クリーンアップ、endpoints.rsに統合）

### レジストリ層

- [x] T019 `llmlb/src/registry/endpoints.rs` にEndpointRegistry実装（インメモリキャッシュ）

### APIハンドラー

- [x] T020 `llmlb/src/api/endpoints.rs` にPOST /api/endpoints ハンドラー
- [x] T021 `llmlb/src/api/endpoints.rs` にGET /api/endpoints ハンドラー
- [x] T022 `llmlb/src/api/endpoints.rs` にGET /api/endpoints/:id ハンドラー
- [x] T023 `llmlb/src/api/endpoints.rs` にPUT /api/endpoints/:id ハンドラー
- [x] T024 `llmlb/src/api/endpoints.rs` にDELETE /api/endpoints/:id ハンドラー
- [x] T025 `llmlb/src/api/endpoints.rs` にPOST /api/endpoints/:id/test ハンドラー
- [x] T026 `llmlb/src/api/endpoints.rs` にPOST /api/endpoints/:id/sync ハンドラー

### APIルーティング

- [x] T027 `llmlb/src/api/mod.rs` にエンドポイントAPIルートを追加

## Phase 3.4: 統合

### ヘルスチェッカー

- [x] T028 `llmlb/src/health/endpoint_checker.rs` にプル型ヘルスチェッカー実装（レイテンシ計測、履歴保存）
- [x] T029 `llmlb/src/health/mod.rs` にEndpointHealthCheckerをエクスポート
- [x] T030 `llmlb/src/main.rs` または `llmlb/src/server.rs` にヘルスチェッカー起動処理追加
- [x] T030a `llmlb/src/health/startup.rs` にロードバランサー起動時の全エンドポイント並列チェック実装

### モデル同期

- [x] T031 `llmlb/src/sync/mod.rs` にモデル同期ロジック実装（GET /v1/models）
- [x] T031a `llmlb/src/sync/parser.rs` にOpenAI/Ollama両形式のレスポンスパーサー実装
- [x] T031b `llmlb/src/sync/capabilities.rs` にモデル名プレフィックスからのcapabilities自動判定実装
- [x] T031c `llmlb/src/sync/mod.rs` にモデル削除同期（差分計算）を追加

### ルーティング統合

- [x] T032 `llmlb/src/api/openai.rs` にEndpointRegistryを利用したルーティング変更
- [x] T032a `llmlb/src/api/openai.rs` にレイテンシベースのエンドポイント選択実装
- [x] T033 `llmlb/src/api/proxy.rs` にエンドポイントへのプロキシ処理追加

### 認可

- [x] T033a `llmlb/src/auth/middleware.rs` および `llmlb/src/api/endpoints.rs` にviewerロール制限を実装（authenticated_middleware + ensure_admin）

## Phase 3.5: 仕上げ

### Unit Tests

- [x] T034 [P] `llmlb/tests/unit/endpoint_status_test.rs` にEndpointStatus遷移のunit test（pending→offline即時遷移含む）
- [x] T035 [P] `llmlb/tests/unit/endpoint_validation_test.rs` にエンドポイントバリデーションのunit test（name UNIQUE含む）
- [x] T035a [P] capabilities自動判定のunit test（`llmlb/src/sync/capabilities.rs`内のinline testsで対応）
- [x] T035b [P] OpenAI/Ollamaレスポンスパーサーのunit test（`llmlb/src/sync/parser.rs`内のinline testsで対応）
- [x] T035c [P] `llmlb/tests/unit/latency_routing_test.rs` にレイテンシベースルーティング選択のunit test

### 旧コード削除

**注意**: T036-T040の完全削除にはダッシュボードの移行が必要。
現在ダッシュボードが `/api/nodes/*` APIを使用しているため、
以下は段階的に実行する:

**Phase A: 廃止APIの削除（SPEC-94621a1f, SPEC-443acc8c対応）**

- [x] T036a `api/error.rs` を作成しAppErrorを移動（nodes.rsから分離）
- [x] T036b POST /api/nodes ルートを削除（ノード自己登録廃止）
- [x] T036c POST /api/health ルートを削除（プッシュ型ヘルスチェック廃止）

**Phase B-0: ダッシュボードのEndpoints API移行**

- [x] T035d `llmlb/src/api/dashboard.rs` に DashboardEndpoint 型と collect_endpoints 関数を追加
- [x] T035e `llmlb/src/web/dashboard/src/lib/api.ts` に endpointsApi を追加し nodesApi を deprecate
- [x] T035f `llmlb/src/web/dashboard/src/components/dashboard/EndpointTable.tsx` を新規作成
- [x] T035g `llmlb/src/web/dashboard/src/components/dashboard/EndpointDetailModal.tsx` を新規作成
- [x] T035h `llmlb/src/web/dashboard/src/components/dashboard/LogViewer.tsx` を endpoints対応に更新（ロードバランサーログのみに簡素化）

**Phase B: 完全削除（ダッシュボード移行後）**

以下はPhase B-0完了後に実行:

- [x] T036 `llmlb/src/db/nodes.rs` を削除（NodeStorage廃止）✅ 2026-01-19完了
- [x] T037 `llmlb/src/db/node_tokens.rs` を削除（NodeToken廃止）
- [x] T038 `llmlb/src/registry/mod.rs` からNodeRegistry関連コードを削除 ✅ 2026-01-19完了
- [x] T039 `llmlb/src/api/nodes.rs` を削除（旧ノードAPI廃止）✅ 2026-01-19完了
- [x] T040 `common/src/protocol.rs` からRegisterRequest/RegisterResponse/HealthCheckRequest削除 ✅ 2026-01-19完了

**完了（2026-01-19）**: xLLMはOpenAI互換APIを提供するため、通常のEndpointとして登録可能。
新規アーキテクチャは不要と判断し、NodeRegistry関連の残存コードをすべて削除完了。

**削除完了項目**:
- `llmlb/src/db/nodes.rs` - 削除済み（NodeStorage）
- `llmlb/src/api/nodes.rs` - 削除済み（旧ノードAPI）
- `llmlb/src/registry/nodes.rs` - 削除済み（NodeRegistry実装）
- `common/src/protocol.rs` - RegisterRequest/RegisterResponse/RegisterStatus/HealthCheckRequest削除
- `llmlb/benches/loadbalancer_bench.rs` - 削除済み（NodeRegistry依存ベンチマーク）

### ドキュメント・検証

- [x] T041 `specs/SPEC-e8e9326e/quickstart.md` の検証ステップを実行
- [x] T042 `README.md` または `README.ja.md` にエンドポイント管理の説明追加

## 依存関係

```text
T001 → T002-T004（マイグレーション後に型・モジュール作成）
T002-T004 → T005-T016d（構造が整ってからテスト作成）
T005-T016d → T017-T027（テスト失敗後に実装）
T017-T019 → T020-T027（DB・レジストリ後にAPI）
T017-T027 → T028-T033a（コア完了後に統合）
T028-T033a → T034-T035c（統合後にunit test）
T017-T033a → T036-T040（新機能動作後に旧コード削除）
T036-T040 → T041-T042（削除後に検証・ドキュメント）
```

## 並列実行例

```text
# Phase 3.1 セットアップ（T002-T004は並列可能）
T001 → [T002, T003, T004]

# Phase 3.2 Contract Tests（すべて並列可能）
[T005, T006, T007, T008, T009, T010, T011]

# Phase 3.2 Integration Tests（すべて並列可能）
[T012, T013, T014, T015, T016, T016a, T016b, T016c, T016d]

# Phase 3.5 Unit Tests（すべて並列可能）
[T034, T035, T035a, T035b, T035c]
```

## 検証チェックリスト

- [x] すべてのAPI契約（7エンドポイント）に対応するcontract testがある
- [x] すべてのユーザーストーリー（5件+追加4件）に対応するintegration testがある
- [x] すべてのテストが実装より先にある（TDD遵守）
- [x] 並列タスクは本当に独立している（異なるファイル）
- [x] 各タスクは正確なファイルパスを指定
- [x] 同じファイルを変更する[P]タスクがない

## タスクサマリー

| Phase | タスク数 | 並列可能 |
|-------|---------|---------|
| 3.1 Setup | 4 | 3 |
| 3.2 Tests | 16 | 16 |
| 3.3 Core | 12 | 0 |
| 3.4 Integration | 12 | 0 |
| 3.5 Polish | 12 | 5 |
| **合計** | **56** | **24** |

---

*TDD必須 - テストが失敗することを確認してから実装を開始*

---

## 完了サマリー（2026-01-18）

### 機能実装: ✅ 完了

本SPECの主要機能は全て実装完了:

| 機能 | 状態 |
|------|------|
| エンドポイント登録API（CRUD） | ✅ |
| ヘルスチェック（プル型） | ✅ |
| モデル同期（/v1/models） | ✅ |
| 接続テスト | ✅ |
| レイテンシベースルーティング | ✅ |
| EndpointCapability基盤 | ✅ |
| ダッシュボード統合 | ✅ |
| 認可（viewerロール制限） | ✅ |

### 旧コード削除（クリーンアップ）: ⏸️ 将来対応

T036, T038, T039, T040は「NodeRegistryの完全廃止」に関するクリーンアップタスク。
これらはxLLMがEndpointとして登録される新しいアーキテクチャが整った後に対応。

**本SPECの機能要件は100%達成済み。**

---

## 追加要件（2026-01-26）: エンドポイントタイプ自動判別機能

### Phase 4.1: セットアップ

- [x] T100 `llmlb/migrations/` に `YYYYMMDDHHMMSS_add_endpoint_type.sql` マイグレーション追加（endpoint_type列、max_tokens列、model_download_tasksテーブル）
- [x] T101 [P] `llmlb/src/types/endpoint.rs` に EndpointType, DownloadStatus, ModelDownloadTask 型定義を追加

### Phase 4.2: テストファースト (TDD) ⚠️ 4.3の前に完了必須

**重要: これらのテストは記述され、実装前に失敗する必要がある**

#### Contract Tests（API契約検証）

- [x] T102 [P] `llmlb/tests/contract/endpoints_type_filter_test.rs` に GET /api/endpoints?type=xllm の contract test
- [x] T103 [P] `llmlb/tests/contract/endpoints_download_test.rs` に POST /api/endpoints/:id/download の contract test
- [x] T104 [P] `llmlb/tests/contract/endpoints_download_progress_test.rs` に GET /api/endpoints/:id/download/progress の contract test
- [x] T105 [P] `llmlb/tests/contract/endpoints_model_info_test.rs` に GET /api/endpoints/:id/models/:model/info の contract test
- [ ] T103a `llmlb/tests/contract/endpoints_download_test.rs` の #[ignore] を解除し、実装に合わせて期待値を確定
- [ ] T104a `llmlb/tests/contract/endpoints_download_progress_test.rs` の #[ignore] を解除し、progressレスポンスを実装に合わせて検証
- [ ] T105a `llmlb/tests/contract/endpoints_model_info_test.rs` の #[ignore] を解除し、model infoレスポンスの期待値を更新

#### Integration Tests（ユーザーストーリー検証）

- [x] T106 [P] `llmlb/tests/integration/endpoint_type_detection_test.rs` にUS6: タイプ自動判別のintegration test（xLLM/Ollama/vLLM/OpenAI互換）
- [x] T107 [P] `llmlb/tests/integration/endpoint_type_filter_test.rs` にUS7: タイプフィルタリングのintegration test
- [x] T108 [P] `llmlb/tests/integration/endpoint_xllm_download_test.rs` にUS8: xLLMモデルダウンロードのintegration test
- [x] T109 [P] `llmlb/tests/integration/endpoint_download_reject_test.rs` にUS8: 非xLLMダウンロード拒否のintegration test
- [x] T110 [P] `llmlb/tests/integration/endpoint_model_metadata_test.rs` にUS9: モデルメタデータ取得のintegration test
- [x] T111 [P] `llmlb/tests/integration/endpoint_type_manual_override_test.rs` にUS11: 手動タイプ指定のintegration test
- [ ] T106a `llmlb/tests/integration/endpoint_type_detection_test.rs` のモックサーバーハーネスを実装し、#[ignore] を解除
- [ ] T107a `llmlb/tests/integration/endpoint_type_filter_test.rs` の #[ignore] を解除し、タイプフィルタの期待値を確定
- [ ] T108a `llmlb/tests/integration/endpoint_xllm_download_test.rs` のダウンロード同期/完了待ちを実装し、#[ignore] を解除
- [ ] T109a `llmlb/tests/integration/endpoint_download_reject_test.rs` の #[ignore] を解除し、拒否理由の期待値を更新
- [ ] T110a `llmlb/tests/integration/endpoint_model_metadata_test.rs` の #[ignore] を解除し、メタデータ期待値を更新
- [ ] T111a `llmlb/tests/integration/endpoint_type_manual_override_test.rs` の #[ignore] を解除し、手動タイプ更新の期待値を確定

### Phase 4.3: コア実装（テストが失敗した後のみ）

#### タイプ判別ロジック

- [x] T112 `llmlb/src/detection/mod.rs` にエンドポイントタイプ判別モジュールを作成
- [x] T113 `llmlb/src/detection/xllm.rs` にxLLM判別ロジック実装（GET /api/system → xllm_version）
- [x] T114 `llmlb/src/detection/ollama.rs` にOllama判別ロジック実装（GET /api/tags）
- [x] T115 `llmlb/src/detection/vllm.rs` にvLLM判別ロジック実装（Server header）
- [x] T116 `llmlb/src/detection/mod.rs` に判別優先順位ロジック実装（xLLM > Ollama > vLLM > OpenAI互換）

#### DB層拡張

- [x] T117 `llmlb/src/db/endpoints.rs` にendpoint_type列のCRUD対応追加
- [x] T118 `llmlb/src/db/endpoints.rs` にmax_tokens列の更新処理追加
- [x] T119 `llmlb/src/db/download_tasks.rs` にModelDownloadTaskStorage CRUD実装

#### APIハンドラー拡張

- [x] T120 `llmlb/src/api/endpoints.rs` にPOST /api/endpoints でタイプ自動判別を統合
- [x] T121 `llmlb/src/api/endpoints.rs` にGET /api/endpoints?type=xxx フィルタリング対応
- [x] T122 `llmlb/src/api/endpoints.rs` にPUT /api/endpoints/:id でタイプ手動変更対応
- [x] T123 `llmlb/src/api/endpoints.rs` にPOST /api/endpoints/:id/download ハンドラー（xLLMタイプ検証）
- [x] T124 `llmlb/src/api/endpoints.rs` にGET /api/endpoints/:id/download/progress ハンドラー
- [x] T125 `llmlb/src/api/endpoints.rs` にGET /api/endpoints/:id/models/:model/info ハンドラー
- [ ] T123b ダウンロードAPIの実装差分（エラーコード/レスポンス形式）をcontract/integrationテストと一致させる
- [ ] T124b progress APIのレスポンス形式をcontract/integrationテストと一致させる
- [ ] T125b model info APIのレスポンス形式をcontract/integrationテストと一致させる

#### xLLMダウンロード連携

- [x] T126 `llmlb/src/xllm/mod.rs` にxLLMクライアントモジュールを作成
- [x] T127 `llmlb/src/xllm/download.rs` にモデルダウンロード要求・進捗取得実装（POST /api/models/download, GET /api/download/progress）

#### モデルメタデータ取得

- [x] T128 `llmlb/src/metadata/mod.rs` にモデルメタデータ取得モジュールを作成
- [x] T129 `llmlb/src/metadata/xllm.rs` にxLLMメタデータ取得実装（GET /api/models/:model/info → context_length）
- [x] T130 `llmlb/src/metadata/ollama.rs` にOllamaメタデータ取得実装（POST /api/show → parameters.num_ctx）
- [ ] T130a メタデータ取得の返却形式（max_tokens/context_length）を統一し、APIレスポンスに反映

---

## 追加要件（2026-02-06）: タイプ判定メタデータの説明責任

### Phase 4.4: テストファースト (TDD)

- [ ] T140 [P] `llmlb/tests/integration/endpoint_type_detection_test.rs` に判定メタデータ（source/reason/detected_at）の検証を追加
- [ ] T141 [P] `llmlb/tests/integration/endpoint_type_manual_override_test.rs` に手動上書き時のsource/reason/detected_at検証を追加
- [ ] T142 [P] `llmlb/tests/integration/endpoint_type_detection_test.rs` に再判別時のメタデータ更新検証を追加

### Phase 4.5: 実装

- [ ] T143 `llmlb/migrations/` に `YYYYMMDDHHMMSS_add_endpoint_type_metadata.sql` を追加（source/reason/detected_at）
- [ ] T144 `llmlb/src/types/endpoint.rs` に EndpointTypeSource とメタデータフィールドを追加
- [ ] T145 `llmlb/src/detection/` に判定理由を返すAPIを追加（タイプと理由の組）
- [ ] T146 `llmlb/src/db/endpoints.rs` に判定メタデータ列のCRUD対応追加
- [ ] T147 `llmlb/src/registry/endpoints.rs` に判定メタデータ更新処理を追加
- [ ] T148 `llmlb/src/api/endpoints.rs` のCreate/Update/Responseにメタデータを追加
- [ ] T149 `llmlb/src/health/endpoint_checker.rs` の再判別でメタデータを更新
- [ ] T150 `llmlb/src/web/dashboard/src/lib/api.ts` にメタデータ型を追加
- [ ] T151 `llmlb/src/web/dashboard/src/components/dashboard/EndpointTable.tsx` と `EndpointDetailModal.tsx` に判定メタデータ表示を追加

### Phase 4.4: 統合

#### 登録フロー統合

- [x] T131 `llmlb/src/api/endpoints.rs` のPOST /api/endpointsにタイプ判別フロー統合（オフライン時はunknown）
- [x] T132 `llmlb/src/health/endpoint_checker.rs` にタイプ再判別ロジック追加（unknown→オンライン時に再判別）

#### モデル同期拡張

- [x] T133 `llmlb/src/sync/mod.rs` にmax_tokens取得・保存を追加（xLLM/Ollamaのみ）

### Phase 4.5: ダッシュボード統合

- [x] T134 [P] `llmlb/src/web/dashboard/src/lib/api.ts` にendpointsApiにタイプフィルタ・ダウンロード・メタデータAPIを追加
- [x] T135 [P] `llmlb/src/web/dashboard/src/components/dashboard/EndpointTable.tsx` にタイプ列を追加
- [x] T136 [P] `llmlb/src/web/dashboard/src/components/dashboard/EndpointDetailModal.tsx` にタイプ表示・ダウンロードUI追加
- [x] T137 [P] `llmlb/src/web/dashboard/src/components/dashboard/ModelDownloadDialog.tsx` を新規作成（xLLMエンドポイント用）
- [x] T152 [P] `llmlb/src/web/dashboard/src/components/dashboard/EndpointTable.tsx` `EndpointDetailModal.tsx` `EndpointPlayground.tsx` にステータス色分け（online/pending/offline/error）を統一
- [x] T153 [P] `llmlb/tests/e2e-playwright/specs/dashboard/dashboard-nodes.spec.ts` にステータスバッジ色分け検証を追加
- [x] T154 [P] `llmlb/src/web/dashboard/src/components/dashboard/EndpointTable.tsx` に `TPS` 列ソート（昇順/降順）を追加し、`aggregate_tps = null` の行を常に末尾に配置
- [x] T155 [P] `llmlb/src/web/dashboard/src/components/dashboard/endpointSorting.ts` を新規作成し、エンドポイント一覧のソートロジック（TPS含む）を共通化
- [x] T156 [P] `llmlb/tests/e2e-playwright/specs/dashboard/endpoint-tps-sort-logic.spec.ts` と `dashboard-nodes.spec.ts` にTPSソートのロジック検証/E2E検証を追加

### Phase 4.6: Unit Tests

- [x] T138 [P] `llmlb/tests/unit/endpoint_type_detection_test.rs` にタイプ判別ロジックのunit test
- [x] T139 [P] `llmlb/tests/unit/endpoint_type_enum_test.rs` にEndpointType列挙型のシリアライズ/デシリアライズtest
- [x] T140 [P] `llmlb/tests/unit/download_status_test.rs` にDownloadStatus遷移のunit test

### Phase 4.7: 検証・ドキュメント

- [x] T141 `specs/SPEC-e8e9326e/quickstart.md` にタイプ判別の検証ステップを追加・実行
- [x] T142 `README.ja.md` にエンドポイントタイプとxLLM連携の説明を追加
- [ ] T143 追加要件のcontract/integrationテストがCIで常時実行可能になったことを確認し、完了報告を更新

## 追加要件の依存関係

```text
T100 → T101（マイグレーション後に型定義）
T101 → T102-T111（構造が整ってからテスト作成）
T102-T111 → T112-T130（テスト失敗後に実装）
T112-T116 → T120（判別ロジック完成後にAPI統合）
T117-T119 → T120-T125（DB層完成後にAPI実装）
T126-T127 → T123-T124（xLLMクライアント後にダウンロードAPI）
T128-T130 → T125, T133（メタデータ取得後にAPI・同期）
T120-T133 → T134-T137（バックエンド完成後にダッシュボード）
T112-T133 → T138-T140（実装完了後にunit test）
T134-T140 → T141-T142（全完了後に検証・ドキュメント）
```

## 追加要件の並列実行例

```text
# Phase 4.2 Contract Tests（すべて並列可能）
[T102, T103, T104, T105]

# Phase 4.2 Integration Tests（すべて並列可能）
[T106, T107, T108, T109, T110, T111]

# Phase 4.3 タイプ判別ロジック（T113-T115は並列可能）
T112 → [T113, T114, T115] → T116

# Phase 4.5 ダッシュボード（すべて並列可能）
[T134, T135, T136, T137, T154, T155, T156]

# Phase 4.6 Unit Tests（すべて並列可能）
[T138, T139, T140]
```

## 追加要件のタスクサマリー

| Phase | タスク数 | 並列可能 |
|-------|---------|---------|
| 4.1 Setup | 2 | 1 |
| 4.2 Tests | 10 | 10 |
| 4.3 Core | 19 | 3 |
| 4.4 Integration | 3 | 0 |
| 4.5 Dashboard | 7 | 7 |
| 4.6 Unit Tests | 3 | 3 |
| 4.7 Docs | 2 | 0 |
| **合計** | **46** | **24** |

---

*TDD必須 - テストが失敗することを確認してから実装を開始*
