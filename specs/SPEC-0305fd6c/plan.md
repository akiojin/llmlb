# 実装計画: ロードバランサー負荷最適化

**機能ID**: `SPEC-0305fd6c` | **日付**: 2025-12-04 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-0305fd6c/spec.md`の機能仕様

## 概要

クライアント増加時のロードバランサー負荷を軽減するため、以下の最適化を実装:

1. **HTTPクライアントプーリング**: 接続再利用によるオーバーヘッド削減
2. **待機機構の改善**: タイムアウト付き待機と段階的バックプレッシャー
3. **ノード選択の最適化**: キャッシュによる計算コスト削減

## 技術コンテキスト

**言語/バージョン**: Rust 1.75+
**主要依存関係**: axum, reqwest, tokio, serde
**ストレージ**: SQLite（既存のリクエスト履歴）
**テスト**: cargo test + 負荷テスト（wrk/k6）
**対象プラットフォーム**: Linux/macOS サーバー
**プロジェクトタイプ**: single（llmlb/ディレクトリ）
**パフォーマンス目標**: 1000 req/s、p95 < 100ms
**制約**: 外部依存最小限、ノード非公開必須
**スケール/スコープ**: 中規模（〜1000 req/s）

## 憲章チェック

**シンプルさ**:

- プロジェクト数: 1（router）✓
- フレームワークを直接使用? ✓（axum/reqwestを直接使用）
- 単一データモデル? ✓（既存のNode/LoadStateを拡張）
- パターン回避? ✓（新規パターン導入なし）

**アーキテクチャ**:

- すべての機能をライブラリとして? ✓（llmlb/src/lib.rs）
- ライブラリリスト: router（ロードバランシング、プロキシ）
- ライブラリごとのCLI: llmlb-cli

**テスト (妥協不可)**:

- RED-GREEN-Refactorサイクルを強制? ✓
- Gitコミットはテストが実装より先に表示? ✓
- 順序: Contract→Integration→E2E→Unitを厳密に遵守? ✓
- 実依存関係を使用? ✓（モックは最小限）
- 禁止: テスト前の実装、REDフェーズのスキップ ✓

**可観測性**:

- 構造化ロギング含む? ✓（tracing使用）
- エラーコンテキスト十分? ✓

**バージョニング**:

- semantic-release使用 ✓

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-0305fd6c/
├── spec.md              # 機能仕様
├── plan.md              # このファイル
└── tasks.md             # タスク分解（/speckit.tasksで生成）
```

### ソースコード (修正対象)

```text
llmlb/src/
├── lib.rs               # AppState拡張（http_client追加）
├── main.rs              # HTTPクライアント初期化
├── balancer/
│   └── mod.rs           # 待機機構改善、ノード選択最適化
└── api/
    ├── proxy.rs         # プロキシ処理更新
    ├── openai.rs        # OpenAI API更新
    ├── models.rs        # モデルAPI更新
    ├── nodes.rs         # ノードAPI更新
    └── logs.rs          # ログAPI更新
```

## Phase 0: リサーチ完了

技術選択は既存のコードベースに基づいて決定済み:

| 決定 | 選択 | 理由 |
|------|------|------|
| HTTPクライアント | reqwest（共有インスタンス） | 既存の依存関係、接続プーリング内蔵 |
| 待機機構 | tokio::time::timeout + AtomicUsize | 既存実装の拡張、外部依存なし |
| キャッシュ | インメモリ（RwLock） | 外部依存なし、単一インスタンス前提 |

## Phase 1: 設計＆契約

### データモデル拡張

**AppState（lib.rs）**:

```rust
pub struct AppState {
    // 既存フィールド...
    pub http_client: reqwest::Client,  // 追加
}
```

**WaitResult（balancer/mod.rs）**:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitResult {
    Ready,
    Timeout,
    CapacityExceeded,
}
```

**AdmissionDecision（balancer/mod.rs）**:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionDecision {
    Accept,
    AcceptWithDelay(Duration),
    Reject,
}
```

### 契約（テストファースト）

**Phase 1テスト（HTTPクライアントプーリング）**:

- `test_app_state_has_shared_http_client`: AppStateにhttp_clientが存在
- `test_http_client_connection_pooling`: 接続が再利用される

**Phase 2テスト（待機機構）**:

- `test_wait_for_ready_timeout`: タイムアウト時にWaitResult::Timeout
- `test_wait_for_ready_ready_immediately`: ready時にWaitResult::Ready
- `test_wait_for_ready_capacity_exceeded`: 上限超過時にWaitResult::CapacityExceeded
- `test_admission_control_accept`: 50%未満でAccept
- `test_admission_control_delay`: 50-80%でAcceptWithDelay
- `test_admission_control_reject`: 80%以上でReject

**Phase 3テスト（ノード選択）**:

- `test_cached_node_selection`: 短時間の連続呼び出しで同一ノード

## Phase 2: タスク計画アプローチ

**タスク生成戦略**:

- 各テスト → テスト作成タスク（RED）
- 各テスト → 実装タスク（GREEN）
- 関連コード → リファクタリングタスク（REFACTOR）

**順序戦略**:

1. HTTPクライアントプーリング（最も効果が高く、複雑さが低い）
2. 待機機構の改善（システム安定性に直結）
3. ノード選択の最適化（追加の最適化）

**TDD順序**:

- テストが実装より先
- 依存関係順序: lib.rs → balancer/mod.rs → proxy.rs/openai.rs

**推定出力**: tasks.mdに15-20個のタスク

## 複雑さトラッキング

*憲章違反なし*

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了（アプローチのみ記述）
- [x] Phase 3: Tasks生成済み（/speckit.tasksコマンド）
- [x] Phase 4: 実装完了
- [x] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み（なし）

---

*憲章 v1.0.0 に基づく - `/memory/constitution.md` 参照*
