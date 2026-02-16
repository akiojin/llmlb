# タスク: ダッシュボード Modelsタブ＆エンドポイント詳細モーダル改善

**入力**: `/specs/SPEC-8795f98f/` の設計ドキュメント
**前提条件**: plan.md, spec.md, research.md, data-model.md

## フォーマット: `[ID] [P?] [Story] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- **[Story]**: このタスクが属するユーザーストーリー (例: US1, US5)

## Phase 1: 基盤 (集約ロジック)

**目的**: ModelsTable コンポーネントが依存する
AggregatedModel 型定義と aggregateModels 純粋関数を実装する

- [ ] T001 [US1] `llmlb/src/web/dashboard/src/lib/api.ts` に
  `AggregatedModel` インターフェースを追加する。
  data-model.md の型定義に従い、
  id, bestStatus, ready, capabilities, endpointCount,
  endpointIds, ownedBy, source, sizeBytes, requiredMemoryBytes,
  maxTokens, tags, description, repo, filename, chatTemplate フィールドを定義する
- [ ] T002 [US1] `llmlb/src/web/dashboard/src/lib/api.ts` に
  `aggregateModels(models: RegisteredModelView[]): AggregatedModel[]`
  純粋関数を実装する。
  集約ロジック:
  (1) name でグループ化
  (2) bestStatus = registered > caching > pending > error の優先順位で最良を選択
  (3) ready = いずれかが true なら true
  (4) capabilities = 全エンドポイントの OR 和集合
  (5) sizeBytes/maxTokens 等 = 最初の非null値
  (6) tags = 全エンドポイントの重複除去済み結合
  (7) endpointCount = グループ内の要素数

---

## Phase 2: ユーザーストーリー5 - エンドポイント詳細モーダルのスクロール対応 (P1)

**目標**: EndpointDetailModal のコンテンツ見切れ問題を修正する

**独立テスト**: モーダルを開き全セクションをスクロールして確認可能

- [ ] T003 [P] [US5] `llmlb/src/web/dashboard/src/components/dashboard/EndpointDetailModal.tsx`
  の `<div className="space-y-6 py-4">` (L233) を
  `<ScrollArea className="max-h-[calc(100vh-12rem)]">` でラップする。
  DialogHeader (L225-L231) と DialogFooter (L536-L544) は
  ScrollArea の外に維持し固定表示とする。
  既存の ScrollArea インポート (L12) はそのまま使用する

**チェックポイント**: モーダルが全ビューポートサイズでスクロール可能

---

## Phase 3: ユーザーストーリー1 - モデル一覧の横断閲覧 (P1) MVP

**目標**: 全エンドポイントのモデルをモデルID単位で集約表示するテーブルを提供する

**独立テスト**: Modelsタブを開きモデル一覧が集約表示されることを確認

### 実装

- [ ] T004 [US1] `llmlb/src/web/dashboard/src/components/dashboard/ModelsTable.tsx`
  を新規作成する。ModelsTable コンポーネントの基本構造を実装する:
  (1) Props: `models: RegisteredModelView[]`, `endpoints: DashboardEndpoint[]`,
  `isLoading: boolean`
  (2) `useMemo` で `aggregateModels(models)` を呼び出し集約データを生成
  (3) Card + CardHeader（タイトル「Models」+ バッジでモデル数表示
  + 手動リフレッシュボタン）
  (4) Table + TableHeader + TableBody でデフォルト6カラム
  (model\_id, lifecycle\_status, ready, capabilities, size\_bytes, owned\_by) を表示
  (5) 空状態: モデル0件時に「No models registered」メッセージ表示
  (6) ローディング状態: isLoading 時にスピナー表示
  (7) capabilities カラムは Lucide アイコンバッジで表示
  (plan.md のアイコンマッピング参照: MessageSquare=Chat, FileText=Completion,
  Layers=Embed, Settings=Tune, Cpu=Infer, Volume2=TTS, Mic=STT, Image=Image)。
  有効な capability のみアイコン表示し、ホバーで Tooltip に名称表示する
  (8) lifecycle\_status は Badge で色分け表示
  (registered=online/緑, caching=pending/黄, pending=pending/黄, error=destructive/赤)
  (9) size\_bytes は既存の `formatBytes()` (lib/utils.ts) で人間可読形式に変換表示
  (10) ready カラムは緑/灰色の丸ドットで表示
  (11) Lucide Icons: Package をタブアイコンとして使用

- [ ] T005 [US1] `llmlb/src/web/dashboard/src/pages/Dashboard.tsx` に
  Modelsタブを追加する:
  (1) `import { ModelsTable } from '@/components/dashboard/ModelsTable'`
  (2) `import { Package } from 'lucide-react'`
  (3) `modelsApi` をインポートし、`useQuery` で
  `modelsApi.getRegistered()` を呼び出す。
  `refetchInterval` は設定しない（手動リフレッシュのみ）
  (4) TabsList の `grid-cols-4` を `grid-cols-5` に変更
  (5) Endpoints の TabsTrigger の直後に
  `<TabsTrigger value="models"><Package /><span>Models</span></TabsTrigger>` を追加
  (6) Endpoints の TabsContent の直後に
  `<TabsContent value="models"><ModelsTable models={modelsData} endpoints={endpointsData}
  isLoading={isLoadingModels} /></TabsContent>` を追加
  (7) ModelsTable に refetch 関数を渡してリフレッシュボタンに接続する

- [ ] T006 [US1] ModelsTable にアコーディオン展開機能を実装する:
  (1) `expandedModels: Set<string>` の state を追加
  (2) 各モデル行の先頭に展開/折畳ボタン（ChevronRight/ChevronDown）を追加
  (3) 展開時: そのモデルの `endpointIds` に対応する
  `endpoints` props からエンドポイント名・ステータスを表示
  (4) 展開時: 各エンドポイントの統計情報を `useQuery` +
  `endpointsApi.getModelStats(endpointId)` でオンデマンド取得
  （`enabled: expandedModels.has(modelId)` で制御）
  (5) 各エンドポイント行に Endpoint Playground 直リンク
  （`#playground/{endpointId}`）を配置
  (6) 統計情報: total\_requests, successful\_requests, failed\_requests を表示
  (7) エンドポイントがオフラインの場合はステータスバッジで明示

**チェックポイント**: Modelsタブで全モデルが集約表示され、
アコーディオン展開でエンドポイント別情報が確認できる

---

## Phase 4: ユーザーストーリー2 - モデルの検索・フィルタ・ソート (P1)

**目標**: テキスト検索、capabilitiesフィルタ、
lifecycle\_statusフィルタ、カラムソートを実装する

**独立テスト**: 各フィルタ・ソート操作で表示結果が正しく絞り込まれることを確認

- [ ] T007 [US2] ModelsTable にテキスト検索を実装する:
  (1) `search: string` の state を追加
  (2) フィルタエリアに `<Input>` + `<Search>` アイコンを配置
  （EndpointTable と同じパターン）
  (3) `useMemo` で `aggregatedModels` を `search` で
  `id` の部分一致フィルタリング（大文字小文字無視）

- [ ] T008 [US2] ModelsTable に lifecycle\_status フィルタを実装する:
  (1) `statusFilter: LifecycleStatus | 'all'` の state を追加
  (2) フィルタエリアに `<Select>` を配置
  （All Status / Registered / Caching / Pending / Error）
  (3) `useMemo` のフィルタチェーンに `statusFilter` 条件を追加

- [ ] T009 [US2] ModelsTable に capabilities フィルタを実装する:
  (1) `capabilityFilters: Record<string, boolean>` の state を追加
  (初期値: 全て false = フィルタなし)
  (2) DropdownMenu + Checkbox で capabilities フィルタUI を配置
  （chat\_completion, completion, embeddings, text\_to\_speech,
  speech\_to\_text, image\_generation）
  (3) `useMemo` のフィルタチェーンに capabilities 条件を追加
  （チェック済み capability を全て持つモデルのみ表示）

- [ ] T010 [US2] ModelsTable にカラムソート機能を実装する:
  (1) `sortField` と `sortDirection` の state を追加
  (EndpointTable の handleSort パターンを踏襲)
  (2) ソート対象カラム: model\_id(文字列), lifecycle\_status(優先順位),
  size\_bytes(数値), owned\_by(文字列)
  (3) TableHead にクリックハンドラと SortIcon コンポーネントを追加
  (4) `useMemo` でソートロジックを実装

**チェックポイント**: 検索・フィルタ・ソートが組み合わせて正しく動作する

---

## Phase 5: ユーザーストーリー3 - カラム表示カスタマイズ (P2)

**目標**: カラムの表示/非表示を切り替えるUIを提供する

**独立テスト**: カラム設定UIでチェックを切り替えるとテーブルのカラムが増減する

- [ ] T011 [US3] ModelsTable にカラム表示/非表示機能を実装する:
  (1) 全14カラムの定義配列を作成:
  `{ key, label, defaultVisible, render }` の形式
  デフォルト表示: model\_id, lifecycle\_status, ready, capabilities,
  size\_bytes, owned\_by
  デフォルト非表示: max\_tokens, source, tags, description,
  repo, filename, required\_memory\_bytes, chat\_template
  (2) `columnVisibility: Record<string, boolean>` の state を追加
  (初期値はデフォルト設定)
  (3) DropdownMenu + DropdownMenuCheckboxItem で
  カラム表示/非表示の切替UIを実装
  （ボタンは「Columns」ラベル + Settings2 アイコン、フィルタエリアの右端に配置）
  (4) TableHeader と TableBody の map 時に
  `columnVisibility[key]` をチェックして表示/非表示を制御
  (5) 非表示カラムの render 関数も定義しておく:
  max\_tokens → 数値表示、source → テキスト、tags → バッジ配列、
  description → truncate付きテキスト、repo → テキスト、
  filename → mono テキスト、required\_memory\_bytes → formatBytes、
  chat\_template → truncate付きテキスト

**チェックポイント**: カラム設定が即座にテーブルに反映される

---

## Phase 6: ユーザーストーリー4 - ModelsタブからPlaygroundへの遷移 (P2)

**目標**: モデル行からLB Playgroundへモデル指定付き遷移を実現する

**独立テスト**: Playgroundボタンクリック後、
LB Playgroundでモデルが自動選択されることを確認

- [ ] T012 [US4] `llmlb/src/web/dashboard/src/App.tsx` の
  `parseHash()` を拡張する:
  (1) Route 型の `lb-playground` に `initialModel?: string` を追加
  (2) `parseHash()` で `#lb-playground?model=xxx` を解析:
  `hash.startsWith('lb-playground')` の場合に
  `?` 以降のクエリパラメータから `model` を抽出
  (3) `<LoadBalancerPlayground>` に `initialModel={route.initialModel}`
  props を渡す

- [ ] T013 [US4] `llmlb/src/web/dashboard/src/pages/LoadBalancerPlayground.tsx`
  に `initialModel` 対応を実装する:
  (1) Props に `initialModel?: string` を追加
  (2) `useEffect` で `initialModel` が指定されかつ
  `modelsData?.data` にそのモデルが存在する場合に
  `setSelectedModel(initialModel)` を呼び出す
  （既存の L330-L343 のモデル選択 useEffect と競合しないよう、
  initialModel がある場合はそちらを優先する）

- [ ] T014 [US4] ModelsTable にPlayground遷移ボタンを実装する:
  (1) 各モデル行のアクション列に Play アイコンのボタンを追加
  (2) クリック時に `window.location.hash = 'lb-playground?model=' + modelId`
  を実行
  (3) `ready === false` のモデルではボタンを `disabled` にする
  (4) ボタンに `title="Open in Playground"` のツールチップを追加

**チェックポイント**: ModelsタブからPlaygroundへの遷移が1クリックで完了し、
モデルが自動選択される

---

## Phase 7: 仕上げ

**目的**: ビルド、品質チェック、最終検証

- [ ] T015 ダッシュボードをビルドし静的アセットを更新する:
  `pnpm --filter @llm/dashboard build` を実行し、
  `llmlb/src/web/static/` の生成物を確認する
- [ ] T016 品質チェックを実行する:
  `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`, markdownlint を全て通過させる
- [ ] T017 quickstart.md の動作確認手順を実行し、
  全機能が正常に動作することを確認する

---

## 依存関係＆実行順序

### フェーズ依存関係

- **Phase 1 (基盤)**: 依存なし、最初に実行
- **Phase 2 (US5 モーダル修正)**: 依存なし、Phase 1 と並列実行可能
- **Phase 3 (US1 モデル一覧)**: Phase 1 完了に依存
- **Phase 4 (US2 検索・フィルタ)**: Phase 3 の T004 完了に依存
- **Phase 5 (US3 カラムカスタマイズ)**: Phase 3 の T004 完了に依存、Phase 4 と並列可能
- **Phase 6 (US4 Playground連携)**: Phase 3 の T004 完了に依存、Phase 4/5 と並列可能
- **Phase 7 (仕上げ)**: 全フェーズ完了に依存

### 並列実行の機会

```text
Phase 1 (T001-T002)  ──┬──→ Phase 3 (T004-T006) ──┬──→ Phase 4 (T007-T010)
                       │                           ├──→ Phase 5 (T011) [P]
Phase 2 (T003) [P] ───┘                           └──→ Phase 6 (T012-T014) [P]
                                                         ↓
                                                   Phase 7 (T015-T017)
```

### タスク内依存

- T002 は T001 の型定義に依存
- T005 は T004 のコンポーネントに依存
- T006 は T004 のコンポーネントに依存
- T007-T010 は T004 のコンポーネントに依存（それぞれ独立して実装可能）
- T013 は T012 の Route 型拡張に依存
- T014 は T012 の hash パラメータ対応に依存
