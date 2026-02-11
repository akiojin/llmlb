# 実装計画: エンドポイント単位リクエスト統計

**機能ID**: `SPEC-76643000` | **日付**: 2026-02-10 | **仕様**: [spec.md](spec.md)
**入力**: `specs/SPEC-76643000/spec.md` の機能仕様

## 概要

ダッシュボードでエンドポイント単位のリクエスト数・成功率・エラー率を
確認できるようにする。endpointsテーブルへのカウンタカラム追加と
日次集計テーブル新設により永続的な統計データを提供し、
エンドポイント一覧テーブルと詳細モーダルの両方で表示する。

主要な技術的アプローチ:

1. SQLiteマイグレーションでendpointsテーブルにカウンタ3列追加
2. endpoint_daily_statsテーブルを新規作成（endpoint×model×date粒度）
3. リクエスト完了時にリアルタイムでカウンタ＋日次集計を更新
4. サーバーローカル時間0:00の日次バッチで集計を確定
5. Dashboard APIにカウンタと日次集計のエンドポイントを追加
6. フロントエンドの一覧テーブルと詳細モーダルを拡張

## 技術コンテキスト

**言語/バージョン**: Rust (backend) + TypeScript/React 19 (frontend)
**主要依存関係**: axum, sqlx, tokio (backend) / React Query v5, Recharts v3, Tailwind CSS, Radix UI (frontend)
**ストレージ**: SQLite (sqlx経由)
**テスト**: cargo test (Rust) + Vitest (frontend予定)
**対象プラットフォーム**: Linux/macOS サーバー
**プロジェクトタイプ**: Web (Rustバックエンド + React SPA フロントエンド)
**パフォーマンス目標**: 日次チャート切替が1秒以内
**制約**: カウンタ更新がリクエスト処理のレイテンシに影響を与えない
**スケール/スコープ**: エンドポイント数10〜100、日次集計レコード数は無制限蓄積

## 憲章チェック

| 原則 | 状態 | 備考 |
|------|------|------|
| I. Router-Nodeアーキテクチャ | 合格 | Router側のみの変更。Node側変更なし |
| II. HTTP/REST通信プロトコル | 合格 | 新規APIエンドポイントはREST準拠 |
| III. テストファースト | 合格 | TDD RED→GREEN→REFACTORで実装 |
| IV. GPU必須ノード登録要件 | N/A | GPU要件に影響なし |
| V. シンプルさと開発者体験 | 合格 | 既存パターンに沿った最小限の変更 |
| VI. LLM最適化 | 合格 | 日次集計APIにページングは不要（期間指定で制限） |
| VII. 可観測性とロギング | 合格 | 統計データの永続化により可観測性が向上 |
| VIII. 認証・アクセス制御 | 合格 | 既存のJWT認証で保護される既存APIパス配下 |
| IX. バージョニング | 合格 | feat: でMINOR bump |

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-76643000/
├── spec.md              # 機能仕様書
├── plan.md              # このファイル
├── research.md          # 技術リサーチ
├── data-model.md        # データモデル定義
├── quickstart.md        # 実装クイックスタート
└── tasks.md             # タスク分解 (/speckit.tasks で生成)
```

### ソースコード変更箇所

```text
llmlb/
├── migrations/
│   └── 014_add_endpoint_request_stats.sql  # 新規: DBマイグレーション
├── src/
│   ├── api/
│   │   ├── dashboard.rs          # 変更: DashboardEndpoint拡張、新APIエンドポイント
│   │   └── openai.rs             # 変更: リクエスト完了時にカウンタ更新呼び出し追加
│   ├── balancer/
│   │   └── mod.rs                # 変更: finish_request系でカウンタ更新
│   ├── db/
│   │   ├── endpoints.rs          # 変更: カウンタ更新関数追加
│   │   ├── endpoint_daily_stats.rs  # 新規: 日次集計CRUD
│   │   ├── mod.rs                # 変更: endpoint_daily_stats モジュール追加
│   │   └── request_history.rs    # 変更なし（クリーンアップはカウンタに影響しない）
│   ├── main.rs                   # 変更: 日次バッチタスク起動追加
│   └── types/
│       └── endpoint.rs           # 変更: Endpoint構造体にカウンタフィールド追加
└── src/web/dashboard/src/
    ├── lib/
    │   └── api.ts                # 変更: DashboardEndpoint型拡張、新API関数
    └── components/dashboard/
        ├── EndpointTable.tsx      # 変更: Requestsカラム追加
        ├── EndpointDetailModal.tsx  # 変更: 統計カード・チャート・モデル別テーブル追加
        └── EndpointRequestChart.tsx  # 新規: Recharts積み上げ棒グラフコンポーネント
```

## 実装フェーズ

### Phase 1: データ基盤（FR-001〜FR-004, FR-012〜FR-014）

**目的**: 永続的なリクエスト統計データの記録基盤を構築する

1. **SQLiteマイグレーション** (`014_add_endpoint_request_stats.sql`)
   - endpointsテーブルにカウンタカラム3列追加:
     `total_requests`, `successful_requests`, `failed_requests` (INTEGER DEFAULT 0)
   - endpoint_daily_statsテーブル新規作成:
     PK=(endpoint_id, model_id, date), カウンタ3列
   - endpoint_daily_statsにはFOREIGN KEY制約を付けない（孤児データ許容のため）

2. **Rust型の拡張** (`types/endpoint.rs`)
   - Endpoint構造体にカウンタフィールド追加
   - EndpointDailyStats構造体新規定義

3. **DB操作** (`db/endpoints.rs`, `db/endpoint_daily_stats.rs`)
   - `increment_request_counters()`: endpointsテーブルのカウンタをアトミックに更新
   - `upsert_daily_stats()`: endpoint_daily_statsをINSERT OR UPDATE
   - `get_daily_stats()`: 期間指定で日次集計を取得
   - `get_model_stats()`: エンドポイント単位のモデル別集計を取得

4. **リクエスト処理フローへの組み込み** (`balancer/mod.rs`, `api/openai.rs`)
   - `finish_request()`/`finish_request_with_tokens()`の末尾でカウンタ更新
   - 非同期で実行し、レイテンシに影響を与えない（tokio::spawn）

5. **日次バッチタスク** (`main.rs`)
   - サーバーローカル時間0:00にrequest_historyから前日分を集計
   - endpoint_daily_statsの値を確定（リアルタイム値との整合性確認）
   - start_cleanup_taskと同じパターンでtokio::spawnで起動

### Phase 2: バックエンドAPI（FR-005〜FR-011）

**目的**: フロントエンドに統計データを提供するAPIを構築する

1. **DashboardEndpoint拡張** (`api/dashboard.rs`)
   - 構造体にカウンタフィールド追加:
     `total_requests`, `successful_requests`, `failed_requests`
   - `collect_endpoints()`でカウンタ値をendpointsテーブルから取得

2. **新規APIエンドポイント**
   - `GET /api/dashboard/endpoints/:id/stats/daily?days=7`
     → 日次集計データ（成功/失敗の日別内訳）
   - `GET /api/dashboard/endpoints/:id/stats/models`
     → モデル別リクエスト数内訳
   - `GET /api/dashboard/endpoints/:id/stats/today`
     → 当日のリクエスト数（日次集計テーブルの当日分）

### Phase 3: フロントエンド一覧テーブル（FR-005〜FR-007）

**目的**: エンドポイント一覧テーブルにRequestsカラムを追加する

1. **TypeScript型の拡張** (`lib/api.ts`)
   - DashboardEndpoint interfaceにカウンタフィールド追加

2. **EndpointTable拡張** (`EndpointTable.tsx`)
   - Requestsカラム追加（ソート可能）
   - 「N (XX.X%)」形式の表示
   - 2段階エラー率ハイライト（≥5%黄/≥20%赤）

### Phase 4: フロントエンド詳細モーダル（FR-008〜FR-011）

**目的**: エンドポイント詳細モーダルに統計セクションを追加する

1. **数値カード4枚** (`EndpointDetailModal.tsx`)
   - 累計リクエスト・今日のリクエスト・成功率・平均レスポンス時間
   - 成功率カードに2段階閾値カラーリング

2. **日次トレンドチャート** (`EndpointRequestChart.tsx` 新規)
   - Recharts BarChart（積み上げ棒グラフ）
   - 7/30/90日タブ切替
   - 成功=緑、失敗=赤
   - データなし時のエンプティステート

3. **モデル別テーブル** (`EndpointDetailModal.tsx`)
   - モデル名・合計・成功・失敗のテーブル

## 複雑さトラッキング

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| 日次バッチタスク追加 | リアルタイムカウンタの整合性を日次で確定するため | リアルタイムのみだとクラッシュ時にデータ不整合の可能性 |
| endpoint_daily_stats新テーブル | endpoint×model×date粒度の永続データが必要 | endpointsカウンタだけではトレンド・モデル別分析ができない |
