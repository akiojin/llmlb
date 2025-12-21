# 実装計画: モデル capabilities に基づくルーティング検証

**機能ID**: `SPEC-32637000` | **日付**: 2025-12-19 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-32637000/spec.md`の機能仕様

## 概要

OpenAI互換APIでモデルを指定して呼び出した際、そのモデルが要求されたAPI（TTS、ASR、画像生成など）に対応しているかをルーターで検証し、非対応の場合はエラーを返す機能を実装する。

主要要件:

- FR-001: 各モデルに対してAPI能力（capabilities）を管理
- FR-002: APIリクエスト時にモデルのcapabilitiesを検証
- FR-003: 非対応の場合は明確なエラーメッセージを返す
- FR-004: ModelTypeからcapabilitiesを自動推定（後方互換性）
- FR-005: `/v1/models`レスポンスにcapabilitiesを含める

## 技術コンテキスト

**言語/バージョン**: Rust 1.75+
**主要依存関係**: axum, serde, tokio
**ストレージ**: SQLite (既存のモデル登録に追加)
**テスト**: cargo test
**対象プラットフォーム**: Linux/macOS/Windows
**プロジェクトタイプ**: single (既存のrouter/common構造に追加)
**パフォーマンス目標**: 既存のルーティング性能を維持
**制約**: 後方互換性必須（capabilities未設定モデルはModelTypeから推定）
**スケール/スコープ**: 既存のモデル登録機能への拡張

## 憲章チェック

**シンプルさ**:

- プロジェクト数: 2 (common, router) - 既存構造を維持 ✅
- フレームワークを直接使用? ✅ axum/serdeを直接使用
- 単一データモデル? ✅ ModelCapability enumのみ追加
- パターン回避? ✅ シンプルなenumと検証関数のみ

**アーキテクチャ**:

- すべての機能をライブラリとして? ✅ common crateにModelCapability追加
- ライブラリリスト: common (ModelCapability), router (検証ロジック)
- ライブラリごとのCLI: N/A (API拡張のみ)
- ライブラリドキュメント: N/A

**テスト (妥協不可)**:

- RED-GREEN-Refactorサイクルを強制? ✅ 必須
- Gitコミットはテストが実装より先に表示? ✅ 必須
- 順序: Contract→Integration→E2E→Unitを厳密に遵守? ✅
- 実依存関係を使用? ✅ 実際のルーターでテスト
- 禁止: テスト前の実装、REDフェーズのスキップ ✅

**可観測性**:

- 構造化ロギング含む? ✅ 既存のtracingを使用
- エラーコンテキスト十分? ✅ "Model 'X' does not support Y"形式

**バージョニング**:

- バージョン番号割り当て済み? ✅ semantic-release使用
- 破壊的変更を処理? ✅ capabilitiesはオプショナルフィールドとして追加

## プロジェクト構造

### ドキュメント (この機能)

```
specs/SPEC-32637000/
├── spec.md              # 機能仕様
├── plan.md              # このファイル
└── tasks.md             # Phase 2 出力 (/speckit.tasks コマンド)
```

### ソースコード変更

```
common/src/types.rs      # ModelCapability enum追加
router/src/registry/models.rs  # ModelInfo.capabilities追加
router/src/api/audio.rs  # TTS/ASR capabilities検証
router/src/api/openai.rs # chat capabilities検証
router/src/api/images.rs # 画像生成capabilities検証
router/src/api/models.rs # /v1/models レスポンス拡張
```

## Phase 0: リサーチ (完了)

技術コンテキストは既存のコードベースから明確:

- **決定**: ModelCapability enumをcommon/src/types.rsに追加
- **理由**: 既存のModelType enumと同様の構造で一貫性がある
- **検討した代替案**: 別crateの作成 → 不要（シンプルなenum追加のみ）

## Phase 1: 設計

### データモデル

```rust
// common/src/types.rs
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    TextGeneration,   // /v1/chat/completions, /v1/completions
    TextToSpeech,     // /v1/audio/speech
    SpeechToText,     // /v1/audio/transcriptions
    ImageGeneration,  // /v1/images/generations
    Vision,           // /v1/chat/completions with images
    Embedding,        // /v1/embeddings
}

impl ModelCapability {
    /// ModelType から推定されるデフォルトの capabilities を返す
    pub fn from_model_type(model_type: ModelType) -> Vec<Self> { ... }
}
```

### API契約

**既存エンドポイントの変更**:

1. `POST /v1/audio/speech` - TextToSpeech capability検証追加
2. `POST /v1/audio/transcriptions` - SpeechToText capability検証追加
3. `POST /v1/chat/completions` - TextGeneration capability検証追加
4. `POST /v1/images/generations` - ImageGeneration capability検証追加
5. `GET /v1/models` - レスポンスにcapabilitiesフィールド追加

**エラーレスポンス形式**:

```json
{
  "error": {
    "message": "Model 'llama-3.1-8b' does not support text-to-speech",
    "type": "invalid_request_error",
    "code": "model_capability_mismatch"
  }
}
```

## Phase 2: タスク計画アプローチ

**タスク生成戦略**:

1. ModelCapability enum追加 (common crate)
2. 各APIエンドポイントにcapabilities検証追加
3. /v1/modelsレスポンス拡張
4. 後方互換性テスト

**TDD順序**:

1. テスト: ModelCapability serialization/deserialization
2. 実装: ModelCapability enum
3. テスト: capabilities検証関数
4. 実装: 検証関数
5. テスト: 各APIエンドポイントでの検証
6. 実装: 各APIハンドラーに検証追加

**推定出力**: tasks.mdに10-15個のタスク

## 複雑さトラッキング

*憲章違反なし*

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了 (アプローチのみ記述)
- [x] Phase 3: Tasks生成済み (/speckit.tasks コマンド)
- [x] Phase 4: 実装完了
- [x] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み (なし)

---

*憲章 v1.0.0 に基づく - `.specify/memory/constitution.md` 参照*
