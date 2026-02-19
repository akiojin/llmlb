# 実装計画: OpenAI互換API完全準拠（Open Responses API対応）

**機能ID**: `SPEC-0f1de549` | **日付**: 2026-01-16 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-0f1de549/spec.md`の機能仕様

## 概要

OpenAI互換APIを100%準拠にする。主な追加機能:

- Open Responses API（/v1/responses）のパススルー対応
- バックエンド対応状況の自動検出
- /v1/modelsへの対応API情報追加
- 非対応バックエンドへの501 Not Implemented返却

**ロードバランサーの役割**: llmlbはロードバランサー/ゲートウェイとして機能し、
API変換は行わない（パススルーのみ）。

## 技術コンテキスト

**言語/バージョン**: Rust 1.75+
**主要依存関係**: axum, reqwest, serde_json, tokio
**ストレージ**: SQLite（既存エンドポイントテーブル拡張）
**テスト**: cargo test + make openai-tests
**対象プラットフォーム**: Linux/macOS サーバー
**プロジェクトタイプ**: single（routerクレート内で完結）
**パフォーマンス目標**: 既存Chat Completions APIと同等
**制約**: パススルーのみ（変換なし）、<100ms オーバーヘッド
**スケール/スコープ**: 既存エンドポイント管理システムを拡張

## 憲章チェック

*ゲート: Phase 0 research前に合格必須。Phase 1 design後に再チェック。*

**シンプルさ**:

- プロジェクト数: 1（router）
- フレームワークを直接使用? ✅ axumを直接使用（ラッパーなし）
- 単一データモデル? ✅ Endpoint構造体を拡張
- パターン回避? ✅ 特別なパターンなし（既存proxy.rs再利用）

**アーキテクチャ**:

- すべての機能をライブラリとして? ✅ router crateに統合
- ライブラリリスト: router（既存）
- ライブラリごとのCLI: 既存CLIを維持
- ライブラリドキュメント: N/A（内部API追加のみ）

**テスト (妥協不可)**:

- RED-GREEN-Refactorサイクルを強制? ✅
- Gitコミットはテストが実装より先に表示? ✅
- 順序: Contract→Integration→E2E→Unitを厳密に遵守? ✅
- 実依存関係を使用? ✅ 実SQLite使用
- Integration testの対象: /v1/responsesエンドポイント、ヘルスチェック拡張
- 禁止: テスト前の実装、REDフェーズのスキップ ✅

**可観測性**:

- 構造化ロギング含む? ✅ tracing使用
- フロントエンドログ → バックエンド? N/A（APIのみ）
- エラーコンテキスト十分? ✅ 501エラーに理由を含める

**バージョニング**:

- バージョン番号割り当て済み? N/A（内部機能）
- 変更ごとにBUILDインクリメント? N/A
- 破壊的変更を処理? N/A（後方互換）

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-0f1de549/
├── plan.md              # このファイル
├── research.md          # Phase 0 出力
├── data-model.md        # Phase 1 出力
├── quickstart.md        # Phase 1 出力
├── contracts/           # Phase 1 出力
└── tasks.md             # Phase 2 出力 (/speckit.tasks)
```

### ソースコード変更対象

```text
llmlb/src/api/
├── mod.rs              # /v1/responsesルート追加
├── responses.rs        # 【新規】Responses APIハンドラー
├── proxy.rs            # 既存パススルー関数再利用
└── openai.rs           # /v1/modelsレスポンス拡張

llmlb/src/types/
└── endpoint.rs         # supports_responses_apiフラグ追加

llmlb/src/sync/
└── capabilities.rs     # Responses API検出ロジック追加

llmlb/src/registry/
└── endpoint_registry.rs # フィルタリング拡張
```

**構造決定**: オプション1（単一プロジェクト）

## Phase 0: アウトライン＆リサーチ

### 技術リサーチ完了

インタビューで以下の決定が完了:

| 項目 | 決定 |
|------|------|
| エラーハンドリング | パススルー（バックエンドエラーはそのまま返却） |
| 認証方式 | 既存APIキー認証を共用 |
| メトリクス | Chat APIと同等の粒度 |
| API優先度 | 両方対等に維持 |
| クライアント移行 | クライアントに任せる |
| Capability通知 | /v1/modelsに追加 |
| ストリーミング | 完全パススルー |
| 非対応バックエンド | 501 Not Implemented返却 |
| MVPスコープ | モデルAPI拡張含む |

### 参照実装の調査

| プロバイダー | Responses API | 確認方法 |
|-------------|---------------|----------|
| Ollama v0.13.3+ | ✅ 対応 | `/v1/responses` エンドポイント |
| vLLM | ✅ 対応 | `/v1/responses` エンドポイント |
| OpenRouter | ✅ 対応 | `/v1/responses` エンドポイント |
| xLLM | 計画中 | 別SPEC |

**出力**: research.md（上記決定事項を文書化）

## Phase 1: 設計＆契約

*前提条件: research.md完了*

### 1. データモデル

**Endpoint構造体拡張**:

```rust
pub struct Endpoint {
    // 既存フィールド...

    /// Responses API対応フラグ
    pub supports_responses_api: bool,
}
```

**SupportedAPIs型**:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SupportedAPI {
    ChatCompletions,
    Responses,
    Embeddings,
}
```

### 2. API契約

**POST /v1/responses**:

- リクエスト: Open Responses API仕様準拠（パススルー）
- レスポンス: バックエンドからの生レスポンス
- エラー: 501（非対応バックエンド）、502（バックエンドエラー転送）
- 認証: 既存APIキー認証

**GET /v1/models 拡張**:

```json
{
  "data": [
    {
      "id": "model-name",
      "object": "model",
      "supported_apis": ["chat_completions", "responses"]
    }
  ]
}
```

### 3. 契約テスト

- `/v1/responses` 基本リクエスト
- `/v1/responses` ストリーミング
- 非対応バックエンドへの501応答
- `/v1/models` supported_apis フィールド

### 4. テストシナリオ

| シナリオ | テスト内容 |
|----------|----------|
| US6: 基本リクエスト | Responses API対応バックエンドへのパススルー |
| US7: ストリーミング | stream=trueでのイベント転送 |
| US8: 非対応処理 | 501エラー返却 |
| US9: 対応確認 | /v1/modelsにsupported_apis |
| US10: 自動検出 | ヘルスチェックでの検出 |

**出力**: data-model.md, contracts/, 失敗するテスト, quickstart.md

## Phase 2: タスク計画アプローチ

*このセクションは/speckit.tasksコマンドが実行することを記述*

**タスク生成戦略**:

- TDD順序: テストが実装より先
- 依存関係順序: データモデル → ヘルスチェック → エンドポイント → モデルAPI
- 並列実行: [P] マーク付きタスクは独立

**タスク概要**:

1. **Setup**: マイグレーション（supports_responses_api列追加）
2. **Contract Tests**: /v1/responsesの契約テスト作成
3. **Core Implementation**:
   - responses.rsハンドラー作成
   - proxy.rs拡張（必要に応じて）
   - endpoint.rs拡張
4. **Integration**: ヘルスチェック拡張、モデルAPI拡張
5. **Polish**: ドキュメント更新

**推定出力**: tasks.mdに15-20個の番号付きタスク

**重要**: このフェーズは/speckit.tasksコマンドで実行

## Phase 3+: 今後の実装

*これらのフェーズは/planコマンドのスコープ外*

**Phase 3**: タスク実行 (/speckit.tasks)
**Phase 4**: 実装（TDDサイクル厳守）
**Phase 5**: 検証（make quality-checks, make openai-tests）

## 複雑さトラッキング

*憲章チェックに正当化が必要な違反がある場合のみ記入*

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| なし | - | - |

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了（アプローチ記述）
- [x] Phase 3: Tasks生成済み（T054-T078）
- [ ] Phase 4: 実装完了
- [ ] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み

---

*憲章 v2.1.1 に基づく - `/memory/constitution.md` 参照*
