# データモデル: SPEC-8795f98f

## フロントエンド型定義

### AggregatedModel（新規・表示用）

モデルID単位で集約した表示用の型。

```typescript
interface AggregatedModel {
  /** モデルID（集約キー） */
  id: string
  /** 最良のlifecycle_status（registered > caching > pending > error） */
  bestStatus: LifecycleStatus
  /** いずれかのエンドポイントでready=trueか */
  ready: boolean
  /** 全エンドポイントのcapabilitiesの和集合 */
  capabilities: ModelCapabilities
  /** 提供エンドポイント数 */
  endpointCount: number
  /** 提供元エンドポイントID一覧 */
  endpointIds: string[]
  /** メタデータ（最初に見つかった非null値を使用） */
  ownedBy?: string
  source?: string
  sizeBytes?: number
  requiredMemoryBytes?: number
  maxTokens?: number | null
  tags: string[]
  description?: string
  repo?: string
  filename?: string
  chatTemplate?: string
}
```

### 既存型（変更なし）

- `OpenAIModel` (api.ts:662-681): APIレスポンスの個別モデル
- `RegisteredModelView` (api.ts:710-726): 表示用変換型
- `ModelCapabilities` (api.ts:650-659): boolean型の8フィールド
- `LifecycleStatus` (api.ts:640): union型
- `ModelStatEntry`: エンドポイント別モデル統計
- `DashboardEndpoint`: エンドポイント情報

### Route型の拡張

```typescript
// 変更前
type Route =
  | { type: 'dashboard' }
  | { type: 'lb-playground' }
  | { type: 'playground'; endpointId: string }

// 変更後
type Route =
  | { type: 'dashboard' }
  | { type: 'lb-playground'; initialModel?: string }
  | { type: 'playground'; endpointId: string }
```

## データフロー

```text
[/api/dashboard/models]
  → OpenAIModelsResponse
  → modelsApi.getRegistered()
  → RegisteredModelView[]
  → aggregateModels()          ← 新規純粋関数
  → AggregatedModel[]
  → ModelsTable コンポーネント

[アコーディオン展開時]
  → endpointsApi.getModelStats(endpointId)  ← 各エンドポイントごと
  → ModelStatEntry[]
  → エンドポイント別統計表示
```

## lifecycle\_status 優先順位

```text
registered (4) > caching (3) > pending (2) > error (1)
```

集約時に最も高い優先度のステータスを `bestStatus` とする。

## capabilities 和集合ロジック

```text
集約後.chat_completion = ep1.chat_completion || ep2.chat_completion || ...
集約後.completion      = ep1.completion      || ep2.completion      || ...
（各フィールドについて同様にOR演算）
```
