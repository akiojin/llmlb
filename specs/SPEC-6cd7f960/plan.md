# SPEC-6cd7f960: 実装計画

## 技術スタック

- **Backend**: Rust (Axum)
- **Frontend**: React 18 + TypeScript + Radix UI + Tailwind CSS
- **Database**: SQLite (既存)

## アーキテクチャ

### 静的モデル定義

```
router/src/supported_models.rs
├── SupportedModel 構造体
├── get_supported_models() 関数
└── 動作確認済みモデルの静的リスト
```

### API層

```
/v0/models (GET)     → 対応モデル一覧 + 状態
/v0/models/pull (POST) → モデルPull
/v1/models (GET)     → 利用可能モデルのみ（変更なし）
```

### UI層

```
ModelsSection.tsx
├── LocalTab (既存機能を維持)
└── ModelHubTab (新規)
    └── ModelCard (Pullボタン付き)
```

## 変更対象ファイル

### Router (Rust)

| ファイル | 変更内容 |
|---------|---------|
| `router/src/supported_models.rs` | 新規作成 - 静的モデル定義 |
| `router/src/lib.rs` | mod supported_models 追加 |
| `router/src/api/models.rs` | list_models拡張、pull_model追加、register削除 |
| `router/src/api/mod.rs` | ルーティング変更 |

### Dashboard (TypeScript)

| ファイル | 変更内容 |
|---------|---------|
| `router/src/web/dashboard/src/lib/api.ts` | 型定義追加、register削除 |
| `router/src/web/dashboard/src/components/models/ModelsSection.tsx` | タブ化 |
| `router/src/web/dashboard/src/components/models/ModelHubTab.tsx` | 新規作成 |
| `router/src/web/dashboard/src/components/models/ModelCard.tsx` | 新規作成 |

## データモデル

### SupportedModel (Rust)

```rust
pub struct SupportedModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub repo: String,
    pub recommended_filename: String,
    pub size_bytes: u64,
    pub required_memory_bytes: u64,
    pub tags: Vec<String>,
    pub capabilities: Vec<String>,
    pub quantization: Option<String>,
    pub parameter_count: Option<String>,
}
```

### ModelWithStatus (API Response)

```rust
pub struct ModelWithStatus {
    #[serde(flatten)]
    pub model: SupportedModel,
    pub status: ModelStatus,
    pub lifecycle_status: Option<LifecycleStatus>,
    pub download_progress: Option<DownloadProgress>,
    pub hf_info: Option<HfInfo>,
}

pub enum ModelStatus {
    Available,
    Downloading,
    Downloaded,
}
```

## HF動的情報取得

- エンドポイント: `https://huggingface.co/api/models/{repo}`
- 取得項目: downloads, likes, lastModified
- キャッシュ: HashMap<String, HfInfo> with TTL 10分
- フォールバック: エラー時は静的情報のみ返す

## 実装フェーズ

### Phase 1: 静的モデル定義とAPI

1. supported_models.rs作成（テスト先行）
2. GET /v0/models拡張
3. POST /v0/models/pull実装
4. HF動的情報取得

### Phase 2: ダッシュボードUI

1. api.ts型定義追加
2. ModelHubTab.tsx作成
3. ModelsSection.tsxタブ化
4. ModelCard.tsx共通化

### Phase 3: 廃止機能削除

1. POST /v0/models/register削除
2. POST /v0/models/discover-gguf削除
3. RegisterDialog削除
4. 関連テスト更新

## テスト戦略

### Unit Tests

- supported_models.rs: get_supported_models()
- models.rs: list_models_with_status(), pull_model()

### Integration Tests

- GET /v0/models レスポンス検証
- POST /v0/models/pull キュー登録検証
- 存在しないmodel_idでのエラー検証

### E2E Tests

- Model Hubタブ表示
- Pullボタンクリック→ダウンロード開始
- Localタブへの反映

## 初期対応モデル

ローカルテスト後に追加予定:

1. Qwen2.5 7B Instruct (Q4_K_M)
2. Llama 3.2 3B Instruct (Q4_K_M)
3. Mistral 7B Instruct (Q4_K_M)
4. Phi-3 Mini (Q4_K_M)
5. Gemma 2 9B (Q4_K_M)
