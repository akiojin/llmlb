# 実装計画: 管理ダッシュボード

**機能ID**: `SPEC-712c20cf` | **日付**: 2025-10-31 | **仕様**: [spec.md](./spec.md)
**入力**: `/ollama-coordinator/specs/SPEC-712c20cf/spec.md`の機能仕様

## 実行フロー (/speckit.plan コマンドのスコープ)
```
1. 入力パスから機能仕様を読み込み ✅
2. 技術コンテキストを記入 (要明確化をスキャン) ✅
3. 憲章チェックセクションを評価 ✅
4. Phase 0 を実行 → research.md ✅
5. Phase 1 を実行 → contracts, data-model.md, quickstart.md ✅
6. 憲章チェックセクションを再評価 ✅
7. Phase 2 を計画 → タスク生成アプローチを記述 ✅
8. 停止 - /speckit.tasks コマンドの準備完了
```

## 概要

WebブラウザからアクセスできるリアルタイムダッシュボードUI。エージェントの状態、リクエスト処理状況、パフォーマンスメトリクスを可視化する。

**依存機能**: SPEC-94621a1f（エージェント自己登録）、SPEC-63acef08（統一APIプロキシ）、SPEC-443acc8c（ヘルスチェック）が実装済みであることが前提。

**技術アプローチ**: Vanilla JS + Chart.js + ポーリング（5秒間隔）でシンプルに開始。将来的にWebSocketへ移行可能。

## 技術コンテキスト
**言語/バージョン**: Rust 1.75+ (backend), HTML5/ES6 (frontend)
**主要依存関係**:
- Backend: Axum (静的ファイル配信), Tokio (非同期処理)
- Frontend: Chart.js 4.x (グラフ描画), Vanilla JavaScript (フレームワークなし)
**ストレージ**: なし（既存のAgentRegistry経由でデータ取得）
**テスト**: cargo test (backend), Jest (frontend, 将来的に)
**対象プラットフォーム**: Linuxサーバー (backend), モダンブラウザ (frontend: Chrome, Firefox, Safari, Edge)
**プロジェクトタイプ**: web（backend + frontend）
**パフォーマンス目標**:
- 初回ロード時間: <2秒
- リアルタイム更新レイテンシ: <100ms
- ダッシュボードAPI応答: <50ms
**制約**:
- ビルドプロセスなし（Vanilla JS）
- 外部CDN依存最小限（Chart.js のみ）
- モバイルレスポンシブ対応
**スケール/スコープ**:
- 同時接続ユーザー: ~10人（管理者）
- 監視エージェント数: ~100台
- 1画面（ダッシュボードのみ、詳細ビューは将来拡張）

## 憲章チェック
*ゲート: Phase 0 research前に合格必須。Phase 1 design後に再チェック。*

**シンプルさ**:
- プロジェクト数: 1 (coordinatorプロジェクト内に統合)
- フレームワークを直接使用? ✅ はい（Axum, Chart.js直接使用、ラッパーなし）
- 単一データモデル? ✅ はい（既存のAgent, AgentStatusモデルを再利用）
- パターン回避? ✅ はい（Repository/UoW不使用、直接AgentRegistry利用）

**アーキテクチャ**:
- すべての機能をライブラリとして? N/A（UI機能のため、静的ファイル + APIエンドポイント）
- ライブラリリスト: なし（既存coordinatorライブラリを拡張）
- ライブラリごとのCLI: N/A
- ライブラリドキュメント: N/A

**テスト (妥協不可)**:
- RED-GREEN-Refactorサイクルを強制? ✅ はい
- Gitコミットはテストが実装より先に表示? ✅ はい
- 順序: Contract→Integration→E2E→Unitを厳密に遵守? ✅ はい
- 実依存関係を使用? ✅ はい（実AgentRegistry、実HTTPサーバー）
- Integration testの対象: ダッシュボードAPIエンドポイント、静的ファイル配信
- 禁止: テスト前の実装、REDフェーズのスキップ ✅ 遵守

**可観測性**:
- 構造化ロギング含む? ✅ はい（tracing使用）
- フロントエンドログ → バックエンド? 将来的に実装（Phase 4）
- エラーコンテキスト十分? ✅ はい（APIエラーレスポンスに詳細含む）

**バージョニング**:
- バージョン番号割り当て済み? ✅ はい（coordinatorのバージョンに従う）
- 変更ごとにBUILDインクリメント? ✅ はい
- 破壊的変更を処理? N/A（新機能のため破壊的変更なし）

## プロジェクト構造

### ドキュメント (この機能)
```
specs/SPEC-712c20cf/
├── plan.md              # このファイル
├── research.md          # Phase 0 出力
├── data-model.md        # Phase 1 出力
├── quickstart.md        # Phase 1 出力
├── contracts/           # Phase 1 出力
│   └── dashboard-api.yaml
└── tasks.md             # Phase 2 出力 (/speckit.tasks コマンド)
```

### ソースコード (リポジトリルート)
```
coordinator/
├── src/
│   ├── api/
│   │   ├── agent.rs          # 既存（エージェント登録API）
│   │   ├── proxy.rs          # 既存（プロキシAPI）
│   │   └── dashboard.rs      # 新規（ダッシュボードAPI）
│   ├── dashboard/            # 新規ディレクトリ
│   │   ├── mod.rs
│   │   ├── stats.rs          # 統計情報集計ロジック
│   │   └── static/
│   │       ├── index.html
│   │       ├── dashboard.js
│   │       └── dashboard.css
│   ├── registry/             # 既存（エージェント管理）
│   └── main.rs               # 既存（ダッシュボードルート追加）
└── tests/
    ├── contract/
    │   └── dashboard_api_test.rs  # 新規
    ├── integration/
    │   └── dashboard_test.rs      # 新規
    └── e2e/
        └── dashboard_workflow_test.rs  # 新規
```

**構造決定**: Webアプリケーション（backend + frontend）だが、単一のcoordinatorプロジェクト内に統合。フロントエンドは静的ファイルとして配信。

## Phase 0: アウトライン＆リサーチ

### 技術コンテキストから抽出した不明点
1. ✅ **Chart.js のバージョンと使用方法** → 4.x, CDN経由で読み込み
2. ✅ **ポーリング実装パターン** → setInterval + fetch API
3. ✅ **Axumでの静的ファイル配信方法** → tower_http::services::ServeDir
4. ✅ **レスポンシブデザイン実装** → CSS Grid + Flexbox + Media Queries

### リサーチ結果

#### 決定1: Chart.js 4.x をCDN経由で使用
- **理由**: ビルドプロセス不要、シンプル、軽量（~200KB）
- **代替案検討**:
  - D3.js: 強力だが学習コスト高い、オーバースペック
  - Recharts: Reactが必要、憲章の「フレームワーク回避」に反する
- **実装詳細**:
  ```html
  <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js"></script>
  ```

#### 決定2: ポーリング（5秒間隔）でリアルタイム更新
- **理由**: 実装簡単、WebSocketは過剰、管理ダッシュボードは低頻度アクセス
- **代替案検討**:
  - WebSocket: 双方向通信は不要、複雑さ増加
  - Server-Sent Events: ブラウザ互換性に課題
- **実装詳細**:
  ```javascript
  const POLL_INTERVAL = 5000; // 5秒
  setInterval(async () => {
    const data = await fetchDashboardData();
    updateCharts(data);
  }, POLL_INTERVAL);
  ```

#### 決定3: Axum + tower_http で静的ファイル配信
- **理由**: 既存のAxumスタックに統合、追加依存最小限
- **代替案検討**:
  - 別途Nginxで配信: インフラ複雑化、単一バイナリ配布を阻害
  - embed_static_files: ビルド時埋め込み、開発時の変更が面倒
- **実装詳細**:
  ```rust
  use tower_http::services::ServeDir;

  let app = Router::new()
      .route("/api/dashboard/agents", get(get_agents))
      .route("/api/dashboard/stats", get(get_stats))
      .nest_service("/dashboard", ServeDir::new("coordinator/src/dashboard/static"));
  ```

#### 決定4: CSS Grid + Flexbox でレスポンシブ対応
- **理由**: モダンCSS、フレームワーク不要、メンテナンス容易
- **代替案検討**:
  - Bootstrap: 不要な機能多い、サイズ大きい
  - Tailwind CSS: ビルドプロセス必要、憲章の「シンプルさ」に反する
- **実装詳細**:
  ```css
  .dashboard-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 1rem;
  }

  @media (max-width: 768px) {
    .dashboard-grid {
      grid-template-columns: 1fr;
    }
  }
  ```

### リサーチ summary

すべての技術選択は憲章の「シンプルさ」原則に準拠。ビルドプロセスなし、外部依存最小限、直接的な実装を優先。

## Phase 1: 設計＆契約

### データモデル (data-model.md)

既存のモデルを再利用し、新規モデルは統計情報のみ：

```rust
// 既存モデル（再利用）
pub struct Agent {
    pub id: String,
    pub hostname: String,
    pub ip_address: String,
    pub ollama_version: String,
    pub status: AgentStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub registered_at: DateTime<Utc>,
}

pub enum AgentStatus {
    Online,
    Offline,
}

// 新規モデル（統計情報）
pub struct DashboardStats {
    pub total_agents: usize,
    pub online_agents: usize,
    pub offline_agents: usize,
    pub total_requests: u64,        // 将来実装（SPEC-589f2df1依存）
    pub avg_response_time_ms: f64,  // 将来実装（SPEC-589f2df1依存）
    pub error_count: u64,           // 将来実装（SPEC-589f2df1依存）
}

pub struct AgentMetrics {
    pub agent_id: String,
    pub cpu_usage: Option<f64>,     // 将来実装（Phase 3）
    pub memory_usage: Option<f64>,  // 将来実装（Phase 3）
    pub active_requests: Option<u32>, // 将来実装（SPEC-589f2df1依存）
}
```

### API契約 (contracts/dashboard-api.yaml)

```yaml
openapi: 3.0.0
info:
  title: Ollama Coordinator Dashboard API
  version: 1.0.0

paths:
  /api/dashboard/agents:
    get:
      summary: エージェント一覧取得
      responses:
        '200':
          description: 成功
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Agent'

  /api/dashboard/stats:
    get:
      summary: システム統計情報取得
      responses:
        '200':
          description: 成功
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/DashboardStats'

  /dashboard:
    get:
      summary: ダッシュボード画面表示
      responses:
        '200':
          description: HTML返却
          content:
            text/html: {}

components:
  schemas:
    Agent:
      type: object
      required:
        - id
        - hostname
        - ip_address
        - ollama_version
        - status
        - last_heartbeat
        - registered_at
      properties:
        id:
          type: string
        hostname:
          type: string
        ip_address:
          type: string
        ollama_version:
          type: string
        status:
          type: string
          enum: [Online, Offline]
        last_heartbeat:
          type: string
          format: date-time
        registered_at:
          type: string
          format: date-time

    DashboardStats:
      type: object
      required:
        - total_agents
        - online_agents
        - offline_agents
      properties:
        total_agents:
          type: integer
        online_agents:
          type: integer
        offline_agents:
          type: integer
        total_requests:
          type: integer
          nullable: true
        avg_response_time_ms:
          type: number
          nullable: true
        error_count:
          type: integer
          nullable: true
```

### クイックスタート (quickstart.md)

```markdown
# 管理ダッシュボード クイックスタート

## 前提条件
- コーディネーターが起動している
- 少なくとも1つのエージェントが登録されている

## ダッシュボードアクセス手順

1. **コーディネーター起動**
   ```bash
   cargo run --bin coordinator
   ```

2. **ブラウザでアクセス**
   ```
   http://localhost:8080/dashboard
   ```

3. **期待される表示**
   - エージェント一覧テーブル
   - システム統計サマリー（登録数、オンライン数）
   - 5秒ごとに自動更新

## テストシナリオ

### シナリオ1: エージェント一覧表示
**前提**: コーディネーターが起動している
**実行**: ブラウザで http://localhost:8080/dashboard にアクセス
**結果**: エージェント一覧が表示される

### シナリオ2: リアルタイム更新
**前提**: ダッシュボードを表示中
**実行**: 新しいエージェントを登録
**結果**: 5秒以内に新しいエージェントが一覧に表示される

### シナリオ3: オフライン検出
**前提**: オンラインエージェントが存在
**実行**: エージェントを停止
**結果**: 60秒後にステータスが「オフライン」に変わる
```

## Phase 2: タスク計画アプローチ
*このセクションは/speckit.tasksコマンドが実行することを記述*

**タスク生成戦略**:
1. **契約テスト** (Contract tests)
   - `/api/dashboard/agents` エンドポイントテスト [P]
   - `/api/dashboard/stats` エンドポイントテスト [P]
   - `/dashboard` 静的ファイル配信テスト [P]

2. **データモデルテスト** (Unit tests)
   - DashboardStats 構造体テスト [P]
   - AgentMetrics 構造体テスト [P]

3. **統合テスト** (Integration tests)
   - エージェント一覧API統合テスト
   - 統計情報API統合テスト
   - 静的ファイル配信統合テスト

4. **実装タスク**
   - DashboardStats モデル実装
   - stats.rs（統計集計ロジック）実装
   - dashboard.rs（APIエンドポイント）実装
   - index.html 実装
   - dashboard.js 実装
   - dashboard.css 実装
   - main.rs にルート追加

5. **E2Eテスト**
   - エンドツーエンドダッシュボードワークフローテスト

6. **ドキュメント**
   - README.md にダッシュボード使用法追加

**順序戦略**:
- TDD順序: 契約テスト → モデルテスト → 統合テスト → 実装 → E2Eテスト
- 並列実行: 契約テストは並列可能 [P]
- 依存関係: モデル → API → フロントエンド

**推定出力**: tasks.mdに約30個のタスク

**重要**: このフェーズは/speckit.tasksコマンドで実行

## Phase 3+: 今後の実装
*これらのフェーズは/planコマンドのスコープ外*

**Phase 3**: タスク実行 (/speckit.tasksコマンドがtasks.mdを作成)
**Phase 4**: 実装 (憲章原則に従ってtasks.mdを実行)
**Phase 5**: 検証 (テスト実行、quickstart.md実行、パフォーマンス検証)

## 複雑さトラッキング
*憲章チェックに正当化が必要な違反がある場合のみ記入*

違反なし。すべての設計は憲章に準拠。

## 進捗トラッキング
*このチェックリストは実行フロー中に更新される*

**フェーズステータス**:
- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了（アプローチのみ記述）
- [ ] Phase 3: Tasks生成済み (/speckit.tasks コマンド)
- [ ] Phase 4: 実装完了
- [ ] Phase 5: 検証合格

**ゲートステータス**:
- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み（逸脱なし）

---
*憲章 v1.0.0 に基づく - `/memory/constitution.md` 参照*
