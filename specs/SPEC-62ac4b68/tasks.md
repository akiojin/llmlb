# タスク: IPアドレスロギング＆クライアント分析

**入力**: `/specs/SPEC-62ac4b68/` の設計ドキュメント
**前提条件**: plan.md, spec.md, research.md, data-model.md, quickstart.md

## フォーマット: `[ID] [P?] [Story] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- **[Story]**: このタスクが属するユーザーストーリー (US1-US8)

---

## Phase 1: 基盤 (全ストーリーの前提条件)

**目的**: DBマイグレーション、Rust構造体拡張、IP正規化ユーティリティなど
全ストーリーが依存するコアインフラを構築する

### テスト (RED)

- [ ] T001 [P] [US1] `llmlb/tests/` にIP正規化関数のユニットテスト作成。
  IPv4パススルー、IPv6パススルー、IPv4-mapped IPv6→IPv4正規化、
  ループバック(::1)の各ケースを検証。テスト失敗を確認。

- [ ] T002 [P] [US1] `llmlb/tests/` にsettingsテーブルCRUDのテスト作成。
  `get_setting`/`set_setting`のラウンドトリップ、
  存在しないキーのNone返却、更新の上書きを検証。テスト失敗を確認。

### 実装 (GREEN)

- [ ] T003 [US1] `llmlb/migrations/017_add_client_ip_tracking.sql` を作成。
  (1) `request_history`に`api_key_id TEXT`カラム追加(ALTER TABLE)、
  (2) `client_ip`にインデックス`idx_request_history_client_ip`追加、
  (3) `api_key_id`にインデックス`idx_request_history_api_key_id`追加、
  (4) `settings`テーブル新規作成(key TEXT PK, value TEXT NOT NULL,
  updated_at TEXT NOT NULL)、
  (5) デフォルト閾値`INSERT INTO settings VALUES('ip_alert_threshold','100',datetime('now'))`。

- [ ] T004 [US1] `llmlb/src/common/protocol.rs` の`RequestResponseRecord`に
  `api_key_id: Option<Uuid>`フィールド追加。
  `#[serde(default, skip_serializing_if = "Option::is_none")]`を付与。
  既存シリアライゼーション互換を維持。

- [ ] T005 [US1] `llmlb/src/common/` にIP正規化ユーティリティ関数を追加。
  `normalize_ip(addr: IpAddr) -> IpAddr`: IPv4-mapped IPv6→IPv4変換。
  `normalize_socket_ip(addr: &SocketAddr) -> IpAddr`: SocketAddrからIP抽出+正規化。
  research.mdの実装パターンに従う。

- [ ] T006 [US1] `llmlb/src/db/settings.rs` を新規作成。
  `SettingsStorage`構造体（SqlitePoolを保持）。
  `get_setting(key: &str) -> Option<String>`: SELECTクエリ。
  `set_setting(key: &str, value: &str)`: INSERT OR REPLACEクエリ。
  `db/mod.rs`にモジュール登録。

- [ ] T007 [US1] `llmlb/src/db/request_history.rs` を更新。
  (1) `RequestHistoryRow`に`api_key_id: Option<String>`追加、
  (2) `save_record`のINSERT文に`api_key_id`カラム追加、
  (3) `TryFrom<RequestHistoryRow>`に`api_key_id`パース追加、
  (4) `RecordFilter`に`client_ip: Option<String>`追加、
  (5) `filter_and_paginate`のSQL構築にclient_ipフィルター条件
  (`client_ip = ?`完全一致)追加。

- [ ] T008 [US1] テスト成功を確認（T001, T002が全てGREEN）。
  `cargo test`で関連テストがパスすることを検証。

**チェックポイント**: DB・構造体・ユーティリティの基盤完了

---

## Phase 2: US1 - リクエスト送信元IPの自動記録 (P1)

**目標**: 全推論ハンドラでクライアントIPとAPIキーIDを記録する
**独立テスト**: APIリクエスト送信後、DBにIPとapi_key_idが記録されることを検証

### テスト (RED)

- [ ] T009 [US1] `llmlb/tests/` にIP記録の統合テスト作成。
  テストサーバーに`/v1/chat/completions`リクエスト送信後、
  `request_history`テーブルの`client_ip`がNULLでないこと、
  `api_key_id`が認証に使用したキーのUUIDと一致することを検証。
  テスト失敗を確認。

### 実装 (GREEN)

- [ ] T010 [US1] `llmlb/src/api/openai.rs` の全推論ハンドラ
  (`chat_completions`, `completions`, `embeddings`,
  audio系, images系)に`ConnectInfo<SocketAddr>`パラメータ追加。
  ハンドラ内で`normalize_socket_ip`を呼び出してIPを取得。
  リクエストエクステンションから`ApiKeyAuthContext`のIDを取得
  (`request.extensions().get::<ApiKeyAuthContext>().map(|ctx| ctx.id)`)。

- [ ] T011 [US1] `llmlb/src/api/openai.rs` の全`RequestResponseRecord`作成箇所
  （約15箇所）で`client_ip: None`→`client_ip: Some(client_ip)`、
  `api_key_id: None`→`api_key_id: api_key_id`に更新。
  `client_ip`と`api_key_id`はハンドラ冒頭で取得した値を使用。

- [ ] T012 [US1] テスト成功を確認（T009がGREEN）。
  `cargo test`で統合テストがパスすることを検証。

**チェックポイント**: リクエスト送信でIP+APIキーが記録される

---

## Phase 3: US2 - リクエスト履歴でのIP表示とフィルター (P1)

**目標**: 既存RequestHistory画面にIPカラムとフィルターを追加
**独立テスト**: 履歴一覧にIPが表示され、IPフィルターが動作する

### テスト (RED)

- [ ] T013 [US2] `llmlb/tests/` にIPフィルターの統合テスト作成。
  `filter_and_paginate`に`client_ip`フィルターを指定し、
  該当IPのレコードのみが返却されることを検証。テスト失敗を確認。

### 実装 (GREEN)

- [ ] T014 [US2] `llmlb/src/api/dashboard.rs` の
  `RequestHistoryQuery`構造体に`client_ip: Option<String>`追加。
  `list_request_responses`ハンドラでクエリパラメータを
  `RecordFilter.client_ip`に渡す。

- [ ] T015 [P] [US2] `llmlb/src/web/dashboard/src/components/dashboard/RequestHistoryTable.tsx`
  を更新。(1) テーブルヘッダーに「Client IP」カラム追加、
  (2) 各行に`client_ip`値を表示（nullの場合は「-」）、
  (3) テーブル上部にIPフィルター用テキスト入力追加、
  (4) フィルター値をAPIクエリパラメータに追加。

- [ ] T016 [P] [US2] `llmlb/src/web/dashboard/src/lib/api.ts` の
  `dashboardApi.getRequestResponses`パラメータに
  `client_ip?: string`オプション追加。

- [ ] T017 [US2] テスト成功を確認（T013がGREEN）。

**チェックポイント**: 履歴一覧でIP表示・フィルターが動作

---

## Phase 4: US3 - Clientsタブ基本分析 (P2)

**目標**: ダッシュボードに「Clients」タブを追加し、IPランキングとバーチャートを表示
**独立テスト**: Clientsタブを開くとIPランキングとバーチャートが表示される

### テスト (RED)

- [ ] T018 [US3] `llmlb/tests/` にIPランキングAPI統合テスト作成。
  `GET /api/dashboard/clients`がIPランキング（リクエスト数降順）を
  ページネーション付きで返すことを検証。
  IPv6の/64グルーピングが正しく動作することも検証。
  テスト失敗を確認。

### 実装 (GREEN)

- [ ] T019 [US3] `llmlb/src/db/request_history.rs` にIPランキング集計クエリ追加。
  `get_client_ip_ranking(hours: u32, page: usize, per_page: usize)
  -> Vec<ClientIpRanking>`。
  SQLで過去N時間のclient_ipをGROUP BYしCOUNTで集計。
  IPv6アドレスはRust側で/64プレフィックスにグルーピング後に集計。
  `ClientIpRanking`レスポンス型を定義（data-model.md参照）。

- [ ] T020 [US3] `llmlb/src/api/dashboard.rs` に
  `GET /api/dashboard/clients`ハンドラ追加。
  クエリパラメータ: `page`(デフォルト1), `per_page`(デフォルト20)。
  `get_client_ip_ranking`を呼び出しJSON返却。

- [ ] T021 [US3] `llmlb/src/api/mod.rs` にClientsルート追加。
  `/api/dashboard/clients`をJWT認証ミドルウェア付きで登録。

- [ ] T022 [P] [US3] `llmlb/src/web/dashboard/src/lib/api.ts` に
  `clientsApi`モジュール追加。
  `getClientRanking(params)`, `getTimeline()`, `getModelDistribution()`,
  `getHeatmap()`, `getClientDetail(ip)`, `getClientApiKeys(ip)`。
  `settingsApi`モジュール追加: `getSetting(key)`, `setSetting(key, value)`。

- [ ] T023 [US3] `llmlb/src/web/dashboard/src/pages/Dashboard.tsx` を更新。
  `TabsList`の`grid-cols-4`→`grid-cols-5`に変更。
  5番目の`TabsTrigger`追加（value="clients", アイコン=Users, ラベル="Clients"）。
  `TabsContent value="clients"`に`<ClientsTab />`配置。

- [ ] T024 [US3] `llmlb/src/web/dashboard/src/components/dashboard/ClientsTab.tsx`
  を新規作成。TanStack Queryで`clientsApi.getClientRanking`をフェッチ。
  レイアウト: `Card`内に`ClientBarChart` + `ClientRankingTable`配置。
  空状態: データ0件時に「リクエストデータがありません」メッセージ表示。
  ローディング: Shimmerスケルトン表示。

- [ ] T025 [P] [US3] `llmlb/src/web/dashboard/src/components/dashboard/ClientBarChart.tsx`
  を新規作成。Recharts `BarChart`で上位10 IPのリクエスト数を表示。
  既存`EndpointRequestChart.tsx`のスタイリングパターンに準拠。
  ResponsiveContainer、ダークモード対応CSS変数使用。

- [ ] T026 [P] [US3] `llmlb/src/web/dashboard/src/components/dashboard/ClientRankingTable.tsx`
  を新規作成。Shadcn `Table`でIPランキング表示。
  カラム: IP, リクエスト数, 最終アクセス, APIキー数。
  ページネーション（20件/ページ、Prev/Nextボタン）。
  行クリックで`onSelectIp`コールバック発火（ドリルダウン用）。

- [ ] T027 [US3] テスト成功を確認（T018がGREEN）。

**チェックポイント**: Clientsタブでランキングテーブル+バーチャートが動作

---

## Phase 5: US4 - 使用パターンの時系列分析 (P2)

**目標**: ユニークIP時系列チャートとモデル分布パイチャートを追加
**独立テスト**: 過去24時間のデータが正しくチャート描画される

### テスト (RED)

- [ ] T028 [P] [US4] `llmlb/tests/` に時系列APIテスト作成。
  `GET /api/dashboard/clients/timeline`が1時間刻みのユニークIP数を返却。
  `GET /api/dashboard/clients/models`がモデル分布を返却。テスト失敗確認。

### 実装 (GREEN)

- [ ] T029 [US4] `llmlb/src/db/request_history.rs` に集計クエリ追加。
  `get_unique_ip_timeline(hours: u32) -> Vec<UniqueIpTimelinePoint>`:
  1時間刻みでCOUNT(DISTINCT client_ip)。
  `get_model_distribution_by_clients(hours: u32) -> Vec<ModelDistribution>`:
  モデル別リクエスト数とパーセンテージ。

- [ ] T030 [US4] `llmlb/src/api/dashboard.rs` に
  `GET /api/dashboard/clients/timeline`と
  `GET /api/dashboard/clients/models`ハンドラ追加。
  `llmlb/src/api/mod.rs`にルート登録。

- [ ] T031 [P] [US4] `llmlb/src/web/dashboard/src/components/dashboard/UniqueIpTimeline.tsx`
  を新規作成。Recharts `LineChart`で24時間×1h刻みのユニークIP推移表示。
  X軸: 時刻, Y軸: ユニークIP数。ResponsiveContainer使用。

- [ ] T032 [P] [US4] `llmlb/src/web/dashboard/src/components/dashboard/ModelDistributionPie.tsx`
  を新規作成。Recharts `PieChart`でモデル利用割合表示。
  ラベル付き、カスタムカラーパレット（chart-1〜chart-5 CSS変数）。

- [ ] T033 [US4] `ClientsTab.tsx`を更新。
  レイアウト中段に`UniqueIpTimeline`と`ModelDistributionPie`を2カラムGrid配置。
  TanStack Queryでtimeline/modelsデータをフェッチ。

- [ ] T034 [US4] テスト成功を確認（T028がGREEN）。

**チェックポイント**: 時系列+パイチャートが動作

---

## Phase 6: US5 - 時間帯×曜日ヒートマップ (P2)

**目標**: CSS Gridベースのカスタムヒートマップを追加
**独立テスト**: 24h×7dのマトリックスがリクエスト密度を色で表現する

### テスト (RED)

- [ ] T035 [US5] `llmlb/tests/` にヒートマップAPIテスト作成。
  `GET /api/dashboard/clients/heatmap`が24×7=168セルのデータを返却。
  各セルに`day_of_week`(0-6), `hour`(0-23), `count`を含む。テスト失敗確認。

### 実装 (GREEN)

- [ ] T036 [US5] `llmlb/src/db/request_history.rs` にヒートマップ集計クエリ追加。
  `get_request_heatmap(hours: u32) -> Vec<HeatmapCell>`:
  SQLで`strftime('%w', timestamp)`(曜日)と`strftime('%H', timestamp)`(時間帯)で
  GROUP BYし、各セルのCOUNTを取得。`HeatmapCell`型を定義。

- [ ] T037 [US5] `llmlb/src/api/dashboard.rs` に
  `GET /api/dashboard/clients/heatmap`ハンドラ追加。ルート登録。

- [ ] T038 [US5] `llmlb/src/web/dashboard/src/components/dashboard/RequestHeatmap.tsx`
  を新規作成。CSS Grid (24cols × 7rows)でヒートマップ描画。
  各セルの背景色: リクエスト数に応じて透明度0.1〜1.0（hsl(var(--chart-1))ベース）。
  行ラベル: 月〜日。列ラベル: 0〜23時。
  セルホバーでツールチップ（曜日・時間・件数）表示。

- [ ] T039 [US5] `ClientsTab.tsx`にヒートマップを配置。
  中段レイアウトに`RequestHeatmap`を`Card`内に配置。

- [ ] T040 [US5] テスト成功を確認（T035がGREEN）。

**チェックポイント**: ヒートマップが動作

---

## Phase 7: US6 - IPドリルダウン詳細ビュー (P2)

**目標**: IPクリックで詳細情報を展開表示する
**独立テスト**: IPクリックでリクエスト履歴・モデル分布・時間帯パターンが表示される

### テスト (RED)

- [ ] T041 [US6] `llmlb/tests/` にIPドリルダウンAPIテスト作成。
  `GET /api/dashboard/clients/{ip}/detail`が特定IPの
  合計リクエスト数、直近リクエスト(最大20件)、モデル分布、
  時間帯パターンを返却。テスト失敗確認。

### 実装 (GREEN)

- [ ] T042 [US6] `llmlb/src/db/request_history.rs` にドリルダウンクエリ追加。
  `get_client_detail(ip: &str, limit: usize) -> ClientDetail`:
  特定IPの合計リクエスト数、初回/最終アクセス時刻、直近リクエスト、
  モデル分布、時間帯別アクティビティを取得。

- [ ] T043 [US6] `llmlb/src/api/dashboard.rs` に
  `GET /api/dashboard/clients/{ip}/detail`ハンドラ追加。
  URLパスのIPをデコードして`get_client_detail`に渡す。ルート登録。

- [ ] T044 [US6] `llmlb/src/web/dashboard/src/components/dashboard/ClientDrilldown.tsx`
  を新規作成。展開型パネル（`ClientRankingTable`の行下に展開）。
  3カラムGrid: (1) リクエスト履歴ミニテーブル(直近20件),
  (2) モデル分布ミニPieChart, (3) 時間帯パターンミニBarChart。
  サマリ: 合計リクエスト数、初回/最終アクセス。

- [ ] T045 [US6] `ClientRankingTable.tsx`を更新。
  行クリックでドリルダウン状態をトグル（selectedIp state）。
  選択行の直下に`ClientDrilldown`コンポーネントを表示。
  TanStack Queryで`clientsApi.getClientDetail(ip)`をフェッチ。

- [ ] T046 [US6] テスト成功を確認（T041がGREEN）。

**チェックポイント**: IPドリルダウンが動作

---

## Phase 8: US7 - APIキーとのクロス分析 (P2)

**目標**: IP×APIキーの組み合わせをドリルダウンに表示する
**独立テスト**: ドリルダウンでAPIキー一覧とリクエスト数が表示される

### テスト (RED)

- [ ] T047 [US7] `llmlb/tests/` にAPIキー別集計テスト作成。
  `GET /api/dashboard/clients/{ip}/api-keys`が特定IPの
  使用APIキー一覧（UUID、キー名、リクエスト数）を返却。
  削除済みキーは名前が取得できない（NULLまたは空文字）。テスト失敗確認。

### 実装 (GREEN)

- [ ] T048 [US7] `llmlb/src/db/request_history.rs` にAPIキー集計クエリ追加。
  `get_client_api_keys(ip: &str) -> Vec<ClientApiKeyUsage>`:
  特定IPの`api_key_id`をGROUP BY、COUNT、
  `api_keys`テーブルとLEFT JOINでキー名取得。
  `ClientApiKeyUsage`型定義（api_key_id, name, request_count）。

- [ ] T049 [US7] `llmlb/src/api/dashboard.rs` に
  `GET /api/dashboard/clients/{ip}/api-keys`ハンドラ追加。ルート登録。

- [ ] T050 [US7] `ClientDrilldown.tsx`を更新。
  APIキー一覧セクションを追加。`clientsApi.getClientApiKeys(ip)`でフェッチ。
  テーブル: APIキー名（削除済みは「削除済み」表示）、リクエスト数。

- [ ] T051 [US7] テスト成功を確認（T047がGREEN）。

**チェックポイント**: APIキークロス分析が動作

---

## Phase 9: US8 - 閾値ベースの異常検知 (P3)

**目標**: 閾値超過IPのハイライト表示とGUI設定を実装する
**独立テスト**: 閾値設定→超過IPハイライト→設定変更→即時反映を検証

### テスト (RED)

- [ ] T052 [US8] `llmlb/tests/` に閾値設定APIテスト作成。
  (1) `GET /api/dashboard/settings/ip_alert_threshold`がデフォルト値100返却、
  (2) `PUT /api/dashboard/settings/ip_alert_threshold`で値更新、
  (3) 再取得で更新値が反映。テスト失敗確認。

- [ ] T053 [P] [US8] `llmlb/tests/` に閾値超過検出テスト作成。
  閾値を5に設定し、同一IPから10件リクエスト後、
  IPランキングAPIの`is_alert`がtrueになることを検証。テスト失敗確認。

### 実装 (GREEN)

- [ ] T054 [US8] `llmlb/src/api/dashboard.rs` に設定APIハンドラ追加。
  `GET /api/dashboard/settings/{key}`: `SettingsStorage.get_setting`呼出し。
  `PUT /api/dashboard/settings/{key}`: `SettingsStorage.set_setting`呼出し。
  ルート登録（JWT認証ミドルウェア付き、PUTはCSRF保護付き）。

- [ ] T055 [US8] `llmlb/src/api/dashboard.rs` の
  `get_client_ranking`ハンドラを更新。
  `SettingsStorage.get_setting("ip_alert_threshold")`で閾値取得。
  各IPの過去1時間リクエスト数と閾値を比較し、
  レスポンスの`is_alert`フラグを設定。

- [ ] T056 [P] [US8] `llmlb/src/web/dashboard/src/components/dashboard/AlertThresholdSettings.tsx`
  を新規作成。数値入力フィールド（現在値を初期表示）。
  保存ボタン（`settingsApi.setSetting`でPUT）。
  保存成功でトースト通知。`Card`内にコンパクトに配置。

- [ ] T057 [US8] `ClientRankingTable.tsx`を更新。
  `is_alert`がtrueの行に`Badge variant="destructive"`で「Alert」表示。
  行の背景色を`bg-destructive/10`に変更。

- [ ] T058 [US8] `ClientsTab.tsx`を更新。
  タブ上部に`AlertThresholdSettings`を配置（折りたたみ可能）。

- [ ] T059 [US8] テスト成功を確認（T052, T053がGREEN）。

**チェックポイント**: 異常検知が動作、閾値変更が即時反映

---

## Phase 10: ビルド・統合・品質チェック

**目的**: ダッシュボード再ビルド、品質チェック全通過

- [ ] T060 ダッシュボードフロントエンドビルド。
  `pnpm --filter @llm/dashboard build` を実行。
  `llmlb/src/web/static/`に生成物が出力されることを確認。

- [ ] T061 `cargo fmt --check` でフォーマットチェック。
  違反があれば`cargo fmt`で修正。

- [ ] T062 [P] `cargo clippy -- -D warnings` でリントチェック。
  警告があれば修正。

- [ ] T063 `cargo test` で全テストパス確認（timeout: 600000ms）。

- [ ] T064 `pnpm dlx markdownlint-cli2 "**/*.md" "!node_modules" "!.git" "!.github" "!.worktrees"`
  でMarkdownリント。違反があれば修正。

- [ ] T065 `.specify/scripts/checks/check-tasks.sh` でタスクチェック。

- [ ] T066 全品質チェック通過後、変更をコミット＆プッシュ。
  コミットメッセージ例: `feat(dashboard): IPアドレスロギング＆クライアント分析機能を追加`。

---

## 依存関係＆実行順序

### フェーズ依存関係

- **Phase 1 (基盤)**: 依存なし、すぐに開始可能
- **Phase 2 (US1)**: Phase 1完了に依存
- **Phase 3 (US2)**: Phase 2完了に依存（IPが記録されないとフィルターできない）
- **Phase 4-8 (US3-US7)**: Phase 2完了に依存、相互は並列可能
  - ただしPhase 7(US6)とPhase 8(US7)はPhase 4(US3)の
    `ClientRankingTable`に依存
- **Phase 9 (US8)**: Phase 4完了に依存（ランキングテーブルが必要）
- **Phase 10 (ビルド)**: 全Phase完了に依存

### 並列実行可能なタスク

```text
Phase 1:  T001 || T002  →  T003, T004, T005, T006 (並列可) → T007 → T008
Phase 4:  T022 || T025 || T026  (フロントエンド並列)
Phase 5:  T031 || T032  (チャート並列)
Phase 9:  T052 || T053  (テスト並列), T056 (フロントエンド並列)
Phase 10: T061 || T062 || T064  (リント並列)
```

### 推奨実行順序（単一開発者）

Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6
→ Phase 7 → Phase 8 → Phase 9 → Phase 10
