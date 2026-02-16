# 実装計画: ダッシュボード Modelsタブ＆エンドポイント詳細モーダル改善

**機能ID**: `SPEC-8795f98f` | **日付**: 2026-02-16 | **仕様**: [spec.md](spec.md)
**入力**: `/specs/SPEC-8795f98f/spec.md` の機能仕様

## 概要

ダッシュボードに「Models」タブを追加し、全エンドポイントのモデルを
モデルID単位で集約したテーブルビューを提供する。
アコーディオン展開でエンドポイント別詳細を表示し、
LB Playgroundへのモデル指定遷移を実現する。
併せて、EndpointDetailModal のコンテンツ見切れ問題を修正する。

## 技術コンテキスト

**言語/バージョン**: TypeScript 5.x (Vite + React 18)
**主要依存関係**: React, TanStack React Query, Radix UI, Tailwind CSS, Lucide Icons
**ストレージ**: N/A（フロントエンドのみ、バックエンドAPIは既存）
**テスト**: Vitest (ユニット)、Playwright (E2E)
**対象プラットフォーム**: Webブラウザ（Chrome/Firefox/Safari）
**プロジェクトタイプ**: Web SPA（ダッシュボード）
**パフォーマンス目標**: 初期表示3秒以内、フィルタ操作200ms以内
**制約**: フロントエンドのみの変更（バックエンドAPI変更なし）
**スケール/スコープ**: 100モデル以下の環境を想定

## 憲章チェック

| 原則 | 準拠状況 | 備考 |
|------|----------|------|
| I. Router-Nodeアーキテクチャ | ✅ 準拠 | フロントエンド変更のみ、アーキテクチャに影響なし |
| II. HTTP/REST通信 | ✅ 準拠 | 既存API `/api/dashboard/models` を使用 |
| III. テストファースト | ✅ 遵守 | TDDサイクル厳守（RED→GREEN→REFACTOR） |
| V. シンプルさと開発者体験 | ✅ 準拠 | 既存UIパターンを踏襲、新規抽象化は最小限 |
| VI. LLM最適化 | ✅ 準拠 | 手動リフレッシュのみ、サーバー負荷最小 |
| VIII. 認証 | ✅ 準拠 | 既存JWT認証（fetchWithAuth）を使用 |

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-8795f98f/
├── spec.md              # 機能仕様
├── plan.md              # このファイル
├── research.md          # 技術リサーチ
├── data-model.md        # データモデル
├── quickstart.md        # クイックスタート
└── tasks.md             # タスク分解 (/speckit.tasks で生成)
```

### ソースコード (変更対象)

```text
llmlb/src/web/dashboard/src/
├── pages/
│   ├── Dashboard.tsx                    # [変更] Modelsタブ追加、grid-cols-4→5
│   └── LoadBalancerPlayground.tsx        # [変更] URLパラメータからモデル自動選択
├── components/
│   ├── dashboard/
│   │   ├── ModelsTable.tsx              # [新規] Modelsタブのメインコンポーネント
│   │   └── EndpointDetailModal.tsx      # [変更] ScrollArea追加
│   └── ui/                              # 既存UIコンポーネント使用（変更なし）
├── lib/
│   └── api.ts                           # [変更なし] 既存API使用
│       # modelsApi.getRegistered() → /api/dashboard/models
│       # endpointsApi.getModelStats(id) → /api/endpoints/{id}/model-stats
└── App.tsx                              # [変更] parseHash拡張（?model=xxx対応）
```

## 設計方針

### 1. ModelsTable コンポーネント設計

**データフロー**:

1. `modelsApi.getRegistered()` で `RegisteredModelView[]` を取得
2. クライアントサイドで `model_id` 単位に集約（`AggregatedModel` 生成）
3. テーブルに集約結果を表示
4. アコーディオン展開時に `endpointsApi.getModelStats(endpointId)` をオンデマンド取得

**集約ロジック（`aggregateModels` 関数）**:

- 入力: `RegisteredModelView[]`
- 出力: `AggregatedModel[]`（モデルID単位で集約）
- ステータス: `registered > caching > pending > error` の優先順位で最良を選択
- capabilities: 全エンドポイントの和集合（OR演算）
- size\_bytes, max\_tokens等: 最初に見つかった非null値を使用
- エンドポイント数: そのモデルIDを提供するエンドポイントの数

**状態管理**:

- 検索テキスト: `useState<string>`
- capabilitiesフィルタ: `useState<Record<string, boolean>>`
- lifecycle\_statusフィルタ: `useState<LifecycleStatus | 'all'>`
- ソートフィールド/方向: `useState`
- カラム可視性: `useState<Record<string, boolean>>`
- 展開中のモデルID: `useState<Set<string>>`

### 2. EndpointDetailModal スクロール修正

**変更方針**: `DialogContent` 内の `<div className="space-y-6 py-4">`
を `ScrollArea` でラップし、`max-h-[calc(100vh-12rem)]` で
ビューポートに収まるようにする。DialogHeader と DialogFooter は
ScrollArea の外に配置して固定表示を維持。

### 3. LB Playground モデル自動選択

**変更方針**:

- `App.tsx` の `parseHash()` を拡張し `#lb-playground?model=xxx`
  のクエリパラメータを解析
- Route型に `initialModel?: string` を追加
- `LoadBalancerPlayground` に `initialModel` props を追加
- `useEffect` で `initialModel` が指定されている場合に
  `setSelectedModel(initialModel)` を実行

### 4. capabilitiesアイコンマッピング

| capability | Lucideアイコン | 略称 |
|------------|---------------|------|
| chat\_completion | `MessageSquare` | Chat |
| completion | `FileText` | Completion |
| embeddings | `Layers` | Embed |
| fine\_tune | `Settings` | Tune |
| inference | `Cpu` | Infer |
| text\_to\_speech | `Volume2` | TTS |
| speech\_to\_text | `Mic` | STT |
| image\_generation | `Image` | Image |

### 5. lifecycle\_statusバッジスタイル

| ステータス | Badge variant | 色 |
|-----------|---------------|-----|
| registered | `online` | 緑 |
| caching | `pending` | 黄 |
| pending | `pending` | 黄 |
| error | `destructive` | 赤 |

既存の `getStatusBadgeVariant()` パターンと統一。

## 複雑さトラッキング

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| なし | - | - |

既存のUIパターン（EndpointTable、TokenStatsSection等）を踏襲し、
新規の抽象化やライブラリ追加は行わない。
集約ロジックは純粋関数として実装し、コンポーネントとは分離する。
