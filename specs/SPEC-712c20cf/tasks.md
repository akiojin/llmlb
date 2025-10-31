# タスク: 管理ダッシュボード

**入力**: `/ollama-coordinator/specs/SPEC-712c20cf/`の設計ドキュメント
**前提条件**: plan.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅, quickstart.md ✅
**ステータス**: 📋 未実装

## 実行フロー
```
1. ✅ plan.mdから技術スタック抽出: Rust + Axum + Vanilla JS + Chart.js
2. ✅ data-model.mdからエンティティ抽出: DashboardStats（新規）, Agent（再利用）
3. ✅ contracts/からエンドポイント抽出: 3エンドポイント（GET /api/dashboard/agents, GET /api/dashboard/stats, GET /dashboard）
4. ✅ quickstart.mdからテストシナリオ抽出: 6シナリオ
5. ✅ カテゴリ別にタスク生成: Setup → Tests → Core → Integration → Polish
6. ✅ 並列実行可能タスクに[P]マーク付与
7. ✅ 依存関係グラフ生成
8. ✅ 検証チェックリスト完了
```

## フォーマット: `[ID] [P?] 説明`
- **[P]**: 並列実行可能（異なるファイル、依存関係なし）
- 説明には正確なファイルパスを含む

## パス規約
- **プロジェクト構造**: coordinator/ プロジェクト内に統合（単一プロジェクト）
- **Backend**: `coordinator/src/`
- **Tests**: `coordinator/tests/`
- **Frontend**: `coordinator/src/dashboard/static/`

---

## Phase 3.1: セットアップ

- [ ] **T001** [P] プロジェクト構造作成: `coordinator/src/dashboard/` ディレクトリと `coordinator/src/dashboard/static/` ディレクトリを作成
- [ ] **T002** [P] Cargo.toml依存関係追加: `tower-http = { version = "0.5", features = ["fs"] }` を `coordinator/Cargo.toml` に追加
- [ ] **T003** [P] モジュール宣言: `coordinator/src/dashboard/mod.rs` を作成し、`pub mod stats;` を宣言

**推定時間**: 30分

---

## Phase 3.2: テストファースト（TDD）⚠️ Phase 3.3の前に完了必須

**重要**: これらのテストは記述され、実装前に失敗する（RED）必要がある

### Contract Tests（契約テスト）

- [ ] **T004** [P] `coordinator/tests/contract/dashboard_agents_api_test.rs` に GET /api/dashboard/agents のcontract test作成
  - リクエスト: GET /api/dashboard/agents
  - 期待レスポンス: 200 OK, JSON配列
  - スキーマ検証: Agent[] (id, hostname, ip_address, ollama_version, status, last_heartbeat, registered_at)

- [ ] **T005** [P] `coordinator/tests/contract/dashboard_stats_api_test.rs` に GET /api/dashboard/stats のcontract test作成
  - リクエスト: GET /api/dashboard/stats
  - 期待レスポンス: 200 OK, JSON object
  - スキーマ検証: DashboardStats (total_agents, online_agents, offline_agents)

- [ ] **T006** [P] `coordinator/tests/contract/dashboard_html_test.rs` に GET /dashboard のcontract test作成
  - リクエスト: GET /dashboard
  - 期待レスポンス: 200 OK, Content-Type: text/html
  - HTML検証: タイトルタグに "Dashboard" が含まれる

### Integration Tests（統合テスト）

- [ ] **T007** `coordinator/tests/integration/dashboard_agents_test.rs` にエージェント一覧API統合テスト作成
  - 前提: AgentRegistryに2つのエージェント登録（1つOnline, 1つOffline）
  - 実行: GET /api/dashboard/agents
  - 検証: 2つのエージェントが返される、ステータスが正しい

- [ ] **T008** `coordinator/tests/integration/dashboard_stats_test.rs` に統計情報API統合テスト作成
  - 前提: AgentRegistryに3つのエージェント登録（2つOnline, 1つOffline）
  - 実行: GET /api/dashboard/stats
  - 検証: total_agents=3, online_agents=2, offline_agents=1

- [ ] **T009** `coordinator/tests/integration/dashboard_static_test.rs` に静的ファイル配信統合テスト作成
  - 前提: index.html, dashboard.js, dashboard.cssが存在
  - 実行: GET /dashboard, GET /dashboard/dashboard.js, GET /dashboard/dashboard.css
  - 検証: すべて200 OK、正しいContent-Type

**推定時間**: 3時間

---

## Phase 3.3: コア実装（テストが失敗した後のみ）

### Models（モデル）

- [ ] **T010** [P] `coordinator/src/dashboard/stats.rs` にDashboardStats構造体実装
  - フィールド: total_agents, online_agents, offline_agents, total_requests (Option), avg_response_time_ms (Option), error_count (Option)
  - メソッド: `from_agents(&[Agent]) -> Self`
  - Unit test: 不変条件（total = online + offline）検証

### Services（サービス層）

- [ ] **T011** [P] `coordinator/src/dashboard/stats.rs` に統計集計ロジック実装
  - `DashboardStats::from_agents()` メソッド実装
  - エージェントリストからOnline/Offline数を集計
  - Unit test: 正しい集計結果検証

### API Endpoints（APIエンドポイント）

- [ ] **T012** `coordinator/src/api/dashboard.rs` 作成とモジュール構造定義
  - `use axum::{Router, routing::get, Json, extract::State};`
  - `use std::sync::Arc;`
  - `use crate::registry::AgentRegistry;`

- [ ] **T013** `coordinator/src/api/dashboard.rs` に GET /api/dashboard/agents エンドポイント実装
  - ハンドラー: `async fn get_agents(State(registry): State<Arc<AgentRegistry>>) -> Json<Vec<Agent>>`
  - AgentRegistryからエージェント一覧取得
  - JSON形式で返却

- [ ] **T014** `coordinator/src/api/dashboard.rs` に GET /api/dashboard/stats エンドポイント実装
  - ハンドラー: `async fn get_stats(State(registry): State<Arc<AgentRegistry>>) -> Json<DashboardStats>`
  - AgentRegistryからエージェント一覧取得
  - `DashboardStats::from_agents()` で統計生成
  - JSON形式で返却

- [ ] **T015** `coordinator/src/api/dashboard.rs` にエラーハンドリング追加
  - 500エラーレスポンス構造化（Error struct）
  - tracing::errorでログ記録

### Frontend（フロントエンド）

- [ ] **T016** [P] `coordinator/src/dashboard/static/index.html` 作成
  - HTML5ボイラープレート
  - タイトル: "Ollama Coordinator Dashboard"
  - Chart.js CDN読み込み: `<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js"></script>`
  - dashboard.css, dashboard.js読み込み
  - コンテナ構造: header, stats cards, agent table, charts

- [ ] **T017** [P] `coordinator/src/dashboard/static/dashboard.css` 作成
  - CSS Grid レイアウト（.dashboard-container, .stats-grid）
  - Flexbox テーブル（.agent-table）
  - レスポンシブ Media Queries（@media max-width: 768px）
  - カラーテーマ: Online（緑 #4CAF50）, Offline（赤 #F44336）

- [ ] **T018** [P] `coordinator/src/dashboard/static/dashboard.js` 作成
  - 定数定義: `const POLL_INTERVAL = 5000;`
  - `fetchDashboardData()`: Promise.all で /api/dashboard/agents と /api/dashboard/stats を並列取得
  - `updateDashboard(data)`: エージェントテーブル更新、統計カード更新、Chart.js更新
  - `startPolling()`: setInterval でポーリング開始
  - エラーハンドリング: try-catch でエラー表示

**推定時間**: 6時間

---

## Phase 3.4: 統合

- [ ] **T019** `coordinator/src/main.rs` にダッシュボードルート追加
  - `use tower_http::services::{ServeDir, ServeFile};`
  - Routerに `/api/dashboard/agents` ルート追加
  - Routerに `/api/dashboard/stats` ルート追加
  - Routerに `/dashboard` 静的ファイル配信追加: `nest_service("/dashboard", ServeDir::new("coordinator/src/dashboard/static"))`

- [ ] **T020** `coordinator/src/api/mod.rs` にdashboardモジュール宣言追加
  - `pub mod dashboard;`

- [ ] **T021** 起動ログに Dashboard URL 追加
  - `tracing::info!("Dashboard available at http://{}:{}/dashboard", addr, port);`

**推定時間**: 1時間

---

## Phase 3.5: 仕上げ

### Unit Tests（ユニットテスト）

- [ ] **T022** [P] `coordinator/src/dashboard/stats.rs` に DashboardStats::from_agents() の unit test追加
  - テストケース1: 空のエージェントリスト → total=0, online=0, offline=0
  - テストケース2: 3エージェント（2 Online, 1 Offline） → total=3, online=2, offline=1
  - テストケース3: 不変条件検証 → total == online + offline

### E2E Tests（エンドツーエンドテスト）

- [ ] **T023** `coordinator/tests/e2e/dashboard_workflow_test.rs` にダッシュボードE2Eテスト作成
  - シナリオ1: 基本アクセス（コーディネーター起動 → /dashboard アクセス → 200 OK）
  - シナリオ2: エージェント一覧表示（エージェント登録 → ダッシュボードアクセス → エージェント表示確認）
  - シナリオ3: リアルタイム更新シミュレーション（初期表示 → 新エージェント追加 → 5秒後に再取得 → 新エージェント確認）

### Performance Tests（パフォーマンステスト）

- [ ] **T024** [P] `coordinator/tests/performance/dashboard_api_bench.rs` に GET /api/dashboard/agents のベンチマーク作成
  - 目標: 平均レスポンスタイム < 50ms
  - 100エージェント登録状態でテスト

- [ ] **T025** [P] `coordinator/tests/performance/dashboard_load_test.rs` にポーリング負荷テスト作成
  - シミュレーション: 10同時接続ユーザー（5秒間隔ポーリング）
  - 目標: 2 req/s の負荷で安定動作

### Documentation（ドキュメント）

- [ ] **T026** [P] `README.md` にダッシュボード使用法セクション追加
  - アクセス方法: `http://localhost:8080/dashboard`
  - スクリーンショット配置準備（後日追加）
  - トラブルシューティングリンク

- [ ] **T027** [P] コードコメント追加とリファクタリング
  - すべてのpublic関数にRustdocコメント追加
  - 複雑なロジックにインラインコメント追加
  - 重複コード削除

**推定時間**: 4時間

---

## 依存関係グラフ

```
Setup (T001-T003)
    ↓
Tests (T004-T009) [TDD - 実装前に失敗する必要がある]
    ↓
Models (T010-T011)
    ↓
API Endpoints (T012-T015)
    ↓
Frontend (T016-T018)
    ↓
Integration (T019-T021)
    ↓
Polish (T022-T027)
```

**並列実行可能**:
- T001, T002, T003 （Setup内）
- T004, T005, T006 （Contract tests）
- T007, T008, T009 （Integration tests - 異なるファイル）
- T010, T011 （Models）
- T016, T017, T018 （Frontend - 異なるファイル）
- T022, T024, T025, T026, T027 （Polish内）

**順次実行必須**:
- T012 → T013 → T014 → T015 （同じファイル: dashboard.rs）
- T019 → T020 → T021 （統合作業）

---

## 並列実行例

### Setup Phase（並列実行）
```bash
# T001, T002, T003を同時実行:
Task 1: "mkdir -p coordinator/src/dashboard/static"
Task 2: "Add tower-http to Cargo.toml"
Task 3: "Create coordinator/src/dashboard/mod.rs"
```

### Contract Tests（並列実行）
```bash
# T004, T005, T006を同時実行:
Task 1: "Write contract test for GET /api/dashboard/agents"
Task 2: "Write contract test for GET /api/dashboard/stats"
Task 3: "Write contract test for GET /dashboard"
```

### Frontend（並列実行）
```bash
# T016, T017, T018を同時実行:
Task 1: "Create index.html"
Task 2: "Create dashboard.css"
Task 3: "Create dashboard.js"
```

---

## タスク統計

| フェーズ | タスク数 | 並列実行可能 | 推定時間 |
|---------|---------|------------|---------|
| Setup | 3 | 3 [P] | 30分 |
| Tests | 6 | 3 [P] | 3時間 |
| Core | 9 | 4 [P] | 6時間 |
| Integration | 3 | 0 | 1時間 |
| Polish | 6 | 5 [P] | 4時間 |
| **合計** | **27** | **15 (56%)** | **約14時間** |

---

## 検証チェックリスト

**契約検証**:
- [x] すべてのcontracts（3エンドポイント）に対応するテストがある
  - GET /api/dashboard/agents → T004
  - GET /api/dashboard/stats → T005
  - GET /dashboard → T006

**エンティティ検証**:
- [x] すべてのentities（1新規モデル）にmodelタスクがある
  - DashboardStats → T010, T011

**TDD順序検証**:
- [x] すべてのテスト（T004-T009）が実装（T010-T018）より先にある

**並列実行検証**:
- [x] 並列タスク（[P]マーク）は本当に独立している（異なるファイル、依存関係なし）

**ファイルパス検証**:
- [x] 各タスクは正確なファイルパスを指定
- [x] 同じファイルを変更する[P]タスクがない

---

## 注意事項

1. **TDD厳守**: T004-T009のテストは必ず実装（T010-T018）の前に作成し、REDフェーズ（失敗）を確認すること
2. **コミット頻度**: 各タスク完了後にコミット＆プッシュ
3. **憲章遵守**: すべての実装は `/memory/constitution.md` の「シンプルさ」「テストファースト」原則に準拠
4. **依存SPEC**: SPEC-94621a1f, SPEC-63acef08, SPEC-443acc8c が実装済みであることを確認
5. **Phase 3実装**: CPU/メモリメトリクスはSPEC-589f2df1（ロードバランシング）依存のため、本SPECでは未実装

---

## 関連ドキュメント

- [機能仕様書](./spec.md)
- [実装計画](./plan.md)
- [データモデル](./data-model.md)
- [技術リサーチ](./research.md)
- [クイックスタート](./quickstart.md)
- [API契約](./contracts/dashboard-api.yaml)
