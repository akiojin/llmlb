# タスク: ルーター主導エンドポイント登録システム

**機能ID**: `SPEC-66555000`
**入力**: `/specs/SPEC-66555000/` の設計ドキュメント
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

- [x] T001 `router/migrations/` に `YYYYMMDDHHMMSS_add_endpoints.sql` マイグレーション追加（endpoints, endpoint_models, endpoint_health_checks）
- [x] T002 [P] `router/src/types/endpoint.rs` に型定義を作成（Endpoint, EndpointStatus, EndpointModel, EndpointHealthCheck）
- [x] T003 [P] `router/src/db/mod.rs` に `endpoints` モジュールを追加
- [x] T004 [P] `router/src/api/mod.rs` に `endpoints` モジュールを追加

## Phase 3.2: テストファースト (TDD) ⚠️ 3.3の前に完了必須

**重要: これらのテストは記述され、実装前に失敗する必要がある**

### Contract Tests（API契約検証）✅ RED完了

- [x] T005 [P] `router/tests/contract/endpoints_post_test.rs` に POST /v0/endpoints の contract test
- [x] T006 [P] `router/tests/contract/endpoints_get_list_test.rs` に GET /v0/endpoints の contract test
- [x] T007 [P] `router/tests/contract/endpoints_get_detail_test.rs` に GET /v0/endpoints/:id の contract test
- [x] T008 [P] `router/tests/contract/endpoints_put_test.rs` に PUT /v0/endpoints/:id の contract test
- [x] T009 [P] `router/tests/contract/endpoints_delete_test.rs` に DELETE /v0/endpoints/:id の contract test
- [x] T010 [P] `router/tests/contract/endpoints_test_test.rs` に POST /v0/endpoints/:id/test の contract test
- [x] T011 [P] `router/tests/contract/endpoints_sync_test.rs` に POST /v0/endpoints/:id/sync の contract test

### Integration Tests（ユーザーストーリー検証）✅ RED完了

- [x] T012 [P] `router/tests/integration/endpoint_registration_test.rs` にUS1: エンドポイント登録のintegration test
- [x] T013 [P] `router/tests/integration/endpoint_health_check_test.rs` にUS2: 稼働状況監視のintegration test
- [x] T014 [P] `router/tests/integration/endpoint_model_sync_test.rs` にUS3: モデル同期のintegration test
- [x] T015 [P] `router/tests/integration/endpoint_connection_test_test.rs` にUS4: 接続テストのintegration test
- [x] T016 [P] `router/tests/integration/endpoint_management_test.rs` にUS5: 管理操作のintegration test
- [x] T016a [P] `router/tests/integration/endpoint_name_uniqueness_test.rs` に名前重複検証のintegration test
- [x] T016b [P] `router/tests/integration/endpoint_latency_routing_test.rs` にレイテンシベースルーティングのintegration test
- [x] T016c [P] `router/tests/integration/endpoint_auto_recovery_test.rs` に自動復旧のintegration test
- [x] T016d [P] `router/tests/integration/endpoint_viewer_access_test.rs` にviewerロール制限のintegration test

## Phase 3.3: コア実装（テストが失敗した後のみ）

### DB層

- [ ] T017 `router/src/db/endpoints.rs` にEndpointStorage CRUD実装（name UNIQUE制約、latency_ms含む）
- [ ] T018 `router/src/db/endpoint_models.rs` にEndpointModelStorage CRUD実装
- [ ] T018a `router/src/db/endpoint_health_checks.rs` にEndpointHealthCheckStorage実装（履歴保存・30日クリーンアップ）

### レジストリ層

- [ ] T019 `router/src/registry/endpoints.rs` にEndpointRegistry実装（インメモリキャッシュ）

### APIハンドラー

- [ ] T020 `router/src/api/endpoints.rs` にPOST /v0/endpoints ハンドラー
- [ ] T021 `router/src/api/endpoints.rs` にGET /v0/endpoints ハンドラー
- [ ] T022 `router/src/api/endpoints.rs` にGET /v0/endpoints/:id ハンドラー
- [ ] T023 `router/src/api/endpoints.rs` にPUT /v0/endpoints/:id ハンドラー
- [ ] T024 `router/src/api/endpoints.rs` にDELETE /v0/endpoints/:id ハンドラー
- [ ] T025 `router/src/api/endpoints.rs` にPOST /v0/endpoints/:id/test ハンドラー
- [ ] T026 `router/src/api/endpoints.rs` にPOST /v0/endpoints/:id/sync ハンドラー

### APIルーティング

- [ ] T027 `router/src/api/mod.rs` にエンドポイントAPIルートを追加

## Phase 3.4: 統合

### ヘルスチェッカー

- [ ] T028 `router/src/health/endpoint_checker.rs` にプル型ヘルスチェッカー実装（レイテンシ計測、履歴保存）
- [ ] T029 `router/src/health/mod.rs` にEndpointHealthCheckerをエクスポート
- [ ] T030 `router/src/main.rs` または `router/src/server.rs` にヘルスチェッカー起動処理追加
- [ ] T030a `router/src/health/startup.rs` にルーター起動時の全エンドポイント並列チェック実装

### モデル同期

- [ ] T031 `router/src/sync/mod.rs` にモデル同期ロジック実装（GET /v1/models）
- [ ] T031a `router/src/sync/parser.rs` にOpenAI/Ollama両形式のレスポンスパーサー実装
- [ ] T031b `router/src/sync/capabilities.rs` にモデル名プレフィックスからのcapabilities自動判定実装
- [ ] T031c `router/src/sync/mod.rs` にモデル削除同期（差分計算）を追加

### ルーティング統合

- [ ] T032 `router/src/api/openai.rs` にEndpointRegistryを利用したルーティング変更
- [ ] T032a `router/src/api/openai.rs` にレイテンシベースのエンドポイント選択実装
- [ ] T033 `router/src/api/proxy.rs` にエンドポイントへのプロキシ処理追加

### 認可

- [ ] T033a `router/src/auth/middleware.rs` にviewerロールのGET制限を追加

## Phase 3.5: 仕上げ

### Unit Tests

- [ ] T034 [P] `router/tests/unit/endpoint_status_test.rs` にEndpointStatus遷移のunit test（pending→offline即時遷移含む）
- [ ] T035 [P] `router/tests/unit/endpoint_validation_test.rs` にエンドポイントバリデーションのunit test（name UNIQUE含む）
- [ ] T035a [P] `router/tests/unit/capabilities_detection_test.rs` にcapabilities自動判定のunit test
- [ ] T035b [P] `router/tests/unit/response_parser_test.rs` にOpenAI/Ollamaレスポンスパーサーのunit test
- [ ] T035c [P] `router/tests/unit/latency_routing_test.rs` にレイテンシベースルーティング選択のunit test

### 旧コード削除

- [ ] T036 `router/src/db/nodes.rs` を削除（NodeStorage廃止）
- [ ] T037 `router/src/db/node_tokens.rs` を削除（NodeToken廃止）
- [ ] T038 `router/src/registry/mod.rs` からNodeRegistry関連コードを削除
- [ ] T039 `router/src/api/nodes.rs` を削除（旧ノードAPI廃止）
- [ ] T040 `common/src/protocol.rs` からRegisterRequest/RegisterResponse削除

### ドキュメント・検証

- [ ] T041 `specs/SPEC-66555000/quickstart.md` の検証ステップを実行
- [ ] T042 `README.md` または `README.ja.md` にエンドポイント管理の説明追加

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
