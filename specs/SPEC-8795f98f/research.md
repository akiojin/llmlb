# 技術リサーチ: SPEC-8795f98f

## 既存APIエンドポイント調査

### /api/dashboard/models (GET)

- **認証**: JWT（fetchWithAuth）
- **レスポンス**: `OpenAIModelsResponse { object: 'list', data: OpenAIModel[] }`
- **実装**: `dashboard::get_models()` → `openai::list_models()` に委譲
- **特記**: 全エンドポイントのモデルをフラットに返す。
  同一model\_idが複数エンドポイントに存在する場合、
  重複して返される（エンドポイント区別なし）

### /api/endpoints/{id}/model-stats (GET)

- **認証**: JWT（fetchWithAuth）
- **レスポンス**: `ModelStatEntry[]`
- **用途**: エンドポイント別のモデル統計
  （total\_requests, successful\_requests, failed\_requests）

### フロントエンドAPI型定義（api.ts）

- `OpenAIModel`: /v1/models レスポンスの個別モデル
- `ModelCapabilities`: boolean型の8フィールド
  （chat\_completion, completion, embeddings, fine\_tune,
  inference, text\_to\_speech, speech\_to\_text, image\_generation）
- `LifecycleStatus`: `'pending' | 'caching' | 'registered' | 'error'`
- `RegisteredModelView`: OpenAIModel の表示用変換型
- `modelsApi.getRegistered()`: OpenAIModel → RegisteredModelView変換済み一覧

## 既存UIパターン調査

### タブ構成（Dashboard.tsx）

- Radix UI Tabs使用（`@radix-ui/react-tabs`）
- `TabsList className="grid w-full grid-cols-4"` → 5に変更が必要
- 各タブにLucideアイコン + レスポンシブテキスト表示パターン

### テーブルパターン（EndpointTable.tsx）

- 手動のuseMemoでフィルタ・ソート・ページネーション
- TanStack Tableは未使用（依存追加不要で既存パターン踏襲可能）
- フィルタ: Input（テキスト検索）+ Select（ドロップダウン）
- ソート: カラムヘッダークリック + ChevronUp/Down アイコン

### モーダルパターン（EndpointDetailModal.tsx）

- Radix UI Dialog使用
- `DialogContent className="max-w-2xl"` で幅固定
- ScrollAreaは部分的に使用（モデルリストのみ、h-32）
- コンテンツ全体にはScrollAreaなし → 見切れの原因

### ルーティング（App.tsx）

- ハッシュベースルーティング
- `parseHash()` で `#`, `#lb-playground`, `#playground/{id}` を解析
- クエリパラメータは未対応 → 拡張が必要

## カラム可視性の実装方法

TanStack Tableを導入せず、既存パターン（useMemo + map）で実装する。

- `columnVisibility: Record<string, boolean>` の状態管理
- テーブルヘッダーとボディのmap時にvisibilityをチェック
- DropdownMenu（既存UIコンポーネント）でチェックボックスUI提供

## 集約ロジックの考慮事項

`modelsApi.getRegistered()` はモデルをフラットに返すが、
**エンドポイント情報（endpoint\_id, endpoint\_name）を含まない**。
`RegisteredModelView`にはエンドポイントIDフィールドがない。

→ 集約は `name`（model\_id）でグループ化するが、
エンドポイント別の情報表示にはAPIの拡張が必要な可能性がある。

**対応方針**:

- `modelsApi.getRegistered()` のデータでモデル一覧＋集約は可能
- アコーディオン展開時のエンドポイント別情報は
  `dashboardApi.getEndpoints()` + `endpointsApi.getModelStats(id)`
  を組み合わせて取得
- エンドポイント一覧は Dashboard.tsx で既に取得済み
  （`endpointsData`）なので、propsで渡す
