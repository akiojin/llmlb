# 実装計画: IPアドレスロギング＆クライアント分析

**機能ID**: `SPEC-62ac4b68` | **日付**: 2026-02-20 | **仕様**: [spec.md](spec.md)
**入力**: `/specs/SPEC-62ac4b68/spec.md` の機能仕様

## 概要

llmlbロードバランサーの推論APIリクエストに対して、送信元IPアドレスと
APIキー識別子を記録し、ダッシュボードの新規「Clients」タブで
可視化・分析する機能を実装する。

既存インフラの大部分が活用可能:
サーバーは`ConnectInfo<SocketAddr>`対応済み、
DBスキーマに`client_ip`カラム存在済み、
Rust構造体に`client_ip`フィールド存在済み。
主な作業はハンドラへの値注入、新規マイグレーション、
ダッシュボードAPI追加、フロントエンド実装。

## 技術コンテキスト

**言語/バージョン**: Rust (axum/sqlx/tokio) + TypeScript (React 19)
**主要依存関係**: axum, sqlx (SQLite), tokio, Recharts 3.7, TanStack Query 5,
Shadcn/Radix UI, Tailwind CSS 4, Lucide React
**ストレージ**: SQLite (`~/.llmlb/load balancer.db`)
**テスト**: cargo test (contract/integration/unit), Playwright (E2E)
**対象プラットフォーム**: macOS/Linux (ローカル/社内サーバー)
**プロジェクトタイプ**: Web (Rustバックエンド + React SPAフロントエンド)
**パフォーマンス目標**: Clientsタブ表示3秒以内 (SC-003)、
1000+ IP環境で5秒以内 (SC-006)
**制約**: 既存API互換性維持、既存ダッシュボードデザイン統一

## 憲章チェック

| 原則 | 適合性 | 備考 |
|------|--------|------|
| I. Router-Nodeアーキテクチャ | 適合 | Router側のみの変更、Node側変更なし |
| II. HTTP/REST通信 | 適合 | 既存APIパターン踏襲 |
| III. テストファースト | 適合 | TDD Red-Green-Refactorサイクル遵守 |
| IV. GPU必須 | N/A | GPU関連変更なし |
| V. シンプルさ | 適合 | 既存インフラ活用、最小限の新規追加 |
| VI. LLM最適化 | 適合 | ページネーション20件/ページ |
| VII. 可観測性 | 適合 | IP記録により可観測性が向上 |
| VIII. 認証 | 適合 | 既存JWT認証と同等のアクセス制御 |

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-62ac4b68/
├── spec.md              # 機能仕様
├── plan.md              # この実装計画
├── research.md          # 技術リサーチ
├── data-model.md        # データモデル設計
├── quickstart.md        # クイックスタートガイド
└── tasks.md             # タスク分解 (/speckit.tasks)
```

### ソースコード (変更対象)

```text
llmlb/
├── migrations/
│   └── 017_add_client_ip_tracking.sql     # 新規: api_key_id, settings, index
├── src/
│   ├── api/
│   │   ├── openai.rs                      # 変更: ConnectInfo抽出、IP/APIキー記録
│   │   ├── dashboard.rs                   # 変更: Clients API追加、履歴にIPフィルター
│   │   └── mod.rs                         # 変更: Clientsルート追加
│   ├── auth/
│   │   └── middleware.rs                  # 参照のみ: ApiKeyAuthContext
│   ├── common/
│   │   └── protocol.rs                   # 変更: api_key_idフィールド追加
│   └── db/
│       ├── request_history.rs            # 変更: IPフィルター、集計クエリ追加
│       └── settings.rs                   # 新規: 設定テーブルCRUD
└── src/web/dashboard/src/
    ├── pages/
    │   └── Dashboard.tsx                  # 変更: Clientsタブ追加
    ├── components/dashboard/
    │   ├── RequestHistoryTable.tsx         # 変更: IPカラム・フィルター追加
    │   ├── ClientsTab.tsx                 # 新規: Clientsタブメイン
    │   ├── ClientRankingTable.tsx          # 新規: IPランキングテーブル
    │   ├── ClientBarChart.tsx              # 新規: IP別バーチャート
    │   ├── UniqueIpTimeline.tsx            # 新規: ユニークIP時系列
    │   ├── ModelDistributionPie.tsx        # 新規: モデル分布パイチャート
    │   ├── RequestHeatmap.tsx              # 新規: 時間帯×曜日ヒートマップ
    │   ├── ClientDrilldown.tsx             # 新規: IPドリルダウン詳細
    │   └── AlertThresholdSettings.tsx      # 新規: 閾値設定GUI
    └── lib/
        └── api.ts                         # 変更: Clients API呼び出し追加
```

## 実装フェーズ

### Phase 1: バックエンド基盤 (P1ストーリー対応)

**目標**: IP記録の基盤を確立し、既存APIでIPを取得・保存できるようにする

1. **マイグレーション017作成**
   - `request_history`に`api_key_id TEXT`カラム追加
   - `client_ip`にインデックス追加 (`idx_request_history_client_ip`)
   - `api_key_id`にインデックス追加
   - `settings`テーブル新規作成 (`key TEXT PRIMARY KEY, value TEXT, updated_at TEXT`)
   - デフォルト閾値設定レコード挿入 (`ip_alert_threshold` = `100`)

2. **protocol.rs拡張**
   - `RequestResponseRecord`に`api_key_id: Option<Uuid>`フィールド追加
   - `skip_serializing_if = "Option::is_none"`で後方互換維持

3. **IP正規化ユーティリティ**
   - IPv4-mapped IPv6 → IPv4正規化関数
   - `SocketAddr` → 正規化済み`IpAddr`変換

4. **openai.rsハンドラ更新**
   - 全推論ハンドラに`ConnectInfo<SocketAddr>`パラメータ追加
   - リクエストエクステンションから`ApiKeyAuthContext`のID取得
   - `RequestResponseRecord`作成時に`client_ip`と`api_key_id`を設定
   - `client_ip: None` → `client_ip: Some(normalized_ip)`に全箇所更新

5. **request_history.rs更新**
   - `save_record`のINSERTに`api_key_id`カラム追加
   - `RequestHistoryRow`に`api_key_id`フィールド追加
   - `RecordFilter`に`client_ip: Option<String>`フィールド追加
   - `filter_and_paginate`のSQL構築にIPフィルター条件追加

### Phase 2: ダッシュボードAPI (P2ストーリー対応)

**目標**: Clientsタブに必要なデータをAPIで提供する

1. **集計クエリ追加** (`request_history.rs`)
   - `get_client_ip_ranking`: IP別リクエスト数ランキング（ページネーション付き）
   - `get_unique_ip_timeline`: 時間帯別ユニークIP数
   - `get_model_distribution_by_clients`: クライアント全体のモデル分布
   - `get_request_heatmap`: 時間帯×曜日のリクエスト密度マトリックス
   - `get_client_detail`: 特定IPの詳細（リクエスト履歴、モデル分布、時間帯パターン）
   - `get_client_api_keys`: 特定IPが使用したAPIキー一覧
   - IPv6の/64グルーピングはSQLのSUBSTR関数で実装

2. **settings.rs新規作成**
   - `SettingsStorage`構造体
   - `get_setting(key)` / `set_setting(key, value)` CRUD

3. **dashboard.rsルート追加**
   - `GET /api/dashboard/clients` - IPランキング（ページネーション）
   - `GET /api/dashboard/clients/timeline` - ユニークIP時系列
   - `GET /api/dashboard/clients/models` - モデル分布
   - `GET /api/dashboard/clients/heatmap` - ヒートマップデータ
   - `GET /api/dashboard/clients/{ip}/detail` - IPドリルダウン
   - `GET /api/dashboard/clients/{ip}/api-keys` - IP別APIキー
   - `GET /api/dashboard/settings/{key}` - 設定取得
   - `PUT /api/dashboard/settings/{key}` - 設定更新

### Phase 3: フロントエンド実装 (P2ストーリー対応)

**目標**: Clientsタブの全UIコンポーネントを実装する

1. **RequestHistoryTable.tsx更新**
   - テーブルにClient IPカラム追加
   - IPフィルター入力フィールド追加

2. **Dashboard.tsx更新**
   - 5番目のタブ「Clients」追加（Usersアイコン）
   - `TabsContent`にClientsTabコンポーネント配置

3. **ClientsTab.tsx新規作成**
   - TanStack Queryでデータフェッチ
   - レイアウト: 上段（バーチャート + パイチャート）、
     中段（時系列 + ヒートマップ）、下段（ランキングテーブル）
   - 空状態メッセージ対応

4. **各チャートコンポーネント新規作成**
   - `ClientBarChart.tsx`: Recharts BarChart（上位10 IP）
   - `UniqueIpTimeline.tsx`: Recharts LineChart（24h、1h間隔）
   - `ModelDistributionPie.tsx`: Recharts PieChart
   - `RequestHeatmap.tsx`: CSS Gridベースのカスタムヒートマップ
     （24時間×7曜日、色の濃淡でリクエスト密度表現）

5. **ClientRankingTable.tsx新規作成**
   - ページネーション（20件/ページ）
   - クリックでドリルダウン展開
   - 閾値超過IPのハイライト表示（警告色Badge）

6. **ClientDrilldown.tsx新規作成**
   - 展開型パネル（テーブル行の下に展開）
   - リクエスト履歴一覧（直近20件）
   - モデル分布ミニパイチャート
   - 時間帯パターンミニバーチャート
   - 使用APIキー一覧

7. **api.ts更新**
   - `clientsApi`モジュール追加（全Clientsエンドポイント）
   - `settingsApi`モジュール追加

### Phase 4: 異常検知 (P3ストーリー対応)

**目標**: 閾値ベースの異常検知とGUI設定を実装する

1. **AlertThresholdSettings.tsx新規作成**
   - 閾値入力フィールド（数値入力）
   - 保存ボタン（PUTリクエスト）
   - 現在の閾値表示

2. **ランキングAPI拡張**
   - `/api/dashboard/clients`レスポンスに`is_alert: bool`フラグ追加
   - 過去1時間のリクエスト数と閾値を比較

3. **ClientRankingTable.tsx更新**
   - `is_alert`がtrueのIPに警告色Badge表示

### Phase 5: ビルド・統合

1. `pnpm --filter @llm/dashboard build` でダッシュボード再ビルド
2. `llmlb/src/web/static/` の生成物をコミット
3. 全品質チェック実行 (`make quality-checks`)

## 複雑さトラッキング

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|-------------------------------|
| カスタムヒートマップ | Rechartsにヒートマップ未搭載 | CSS Gridで最小実装、外部ライブラリ追加を回避 |
| settingsテーブル新設 | 閾値のDB永続化（FR-018） | 環境変数では再起動が必要でFR-017に違反 |
