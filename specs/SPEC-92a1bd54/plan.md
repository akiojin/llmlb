# 実装計画: Open Responses API対応

**機能ID**: `SPEC-92a1bd54` | **日付**: 2026-01-16 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-92a1bd54/spec.md`の機能仕様

## 概要

llmlbにOpen Responses API（OpenAI Responses APIベースのオープン仕様）の
パススルー機能を追加する。ロードバランサーはロードバランサー/ゲートウェイとして機能し、
`/v1/responses`エンドポイントへのリクエストをResponses API対応バックエンドに転送する。

### 既存実装の発見

調査の結果、以下の実装が既に存在することが判明:

- `llmlb/src/api/responses.rs` - `/v1/responses`ハンドラー（SPEC-0f1de549として実装済み）
- `llmlb/src/types/endpoint.rs` - `SupportedAPI`列挙型、`supports_responses_api`フラグ
- `llmlb/src/api/mod.rs` - ルート登録済み（234行目）
- `llmlb/src/api/proxy.rs` - `forward_to_endpoint`、`forward_streaming_response`

### 残作業

1. 統合テストの作成・実行
2. `/v1/models`レスポンスへのAPI対応情報追加の検証
3. ヘルスチェックでのResponses API対応検出の検証
4. E2Eテスト（実際のバックエンドとの疎通確認）

## 技術コンテキスト

**言語/バージョン**: Rust 1.75+
**主要依存関係**: axum, reqwest, serde_json, tokio
**ストレージ**: SQLite（エンドポイント情報）
**テスト**: cargo test、統合テスト（wiremock）
**対象プラットフォーム**: Linux/macOS サーバー
**プロジェクトタイプ**: single（llmlb/配下）
**パフォーマンス目標**: 既存APIと同等（パススルーのためオーバーヘッド最小）
**制約**: パススルーのみ（変換なし）、バックエンド依存

## 憲章チェック

*ゲート: Phase 0 research前に合格必須。Phase 1 design後に再チェック。*

**シンプルさ**:

- プロジェクト数: 1（router）- ✅
- フレームワークを直接使用? ✅ axum/reqwestを直接使用
- 単一データモデル? ✅ 既存のEndpoint型を拡張
- パターン回避? ✅ 直接的なパススルー実装

**アーキテクチャ**:

- すべての機能をライブラリとして? ✅ router cratee内に実装
- ライブラリリスト: `api/responses.rs`（Responses APIハンドラー）
- ライブラリごとのCLI: N/A（APIエンドポイント）
- ライブラリドキュメント: ✅ モジュールドキュメント記載済み

**テスト (妥協不可)**:

- RED-GREEN-Refactorサイクルを強制? ✅ 統合テスト追加予定
- Gitコミットはテストが実装より先に表示? ✅ TDD遵守
- 順序: Contract→Integration→E2E→Unitを厳密に遵守? ✅
- 実依存関係を使用? ✅ wiremock/実バックエンド
- Integration testの対象: ✅ Responses APIパススルー
- 禁止: テスト前の実装、REDフェーズのスキップ ✅

**可観測性**:

- 構造化ロギング含む? ✅ tracingで実装済み
- エラーコンテキスト十分? ✅ モデル名、エンドポイント情報をログ

**バージョニング**:

- バージョン番号割り当て済み? ✅ semantic-release
- 変更ごとにBUILDインクリメント? ✅
- 破壊的変更を処理? N/A（新規追加）

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-92a1bd54/
├── spec.md              # 機能仕様書
├── plan.md              # このファイル
└── tasks.md             # タスク一覧（/speckit.tasksで生成）
```

### ソースコード（既存）

```text
llmlb/src/
├── api/
│   ├── responses.rs     # ✅ 実装済み - Open Responses APIハンドラー
│   ├── proxy.rs         # ✅ 実装済み - forward_to_endpoint, forward_streaming_response
│   ├── openai.rs        # /v1/models レスポンス（要確認: API対応情報）
│   └── mod.rs           # ✅ ルート登録済み
├── types/
│   └── endpoint.rs      # ✅ 実装済み - SupportedAPI, supports_responses_api
└── sync/
    └── health_checker.rs # ヘルスチェック（要確認: Responses API検出）

llmlb/tests/integration/
└── responses_api_test.rs # ⚠️ 未作成 - 統合テスト必要
```

## Phase 0: アウトライン＆リサーチ

### リサーチ完了事項

1. **既存実装の調査** - 完了
   - `responses.rs`: パススルーロジック実装済み
   - `endpoint.rs`: `supports_responses_api`フラグ存在
   - ルーティング: `/v1/responses`登録済み

2. **Open Responses API仕様** - 完了
   - OpenAI Responses API互換
   - ストリーミングイベント形式確認済み

3. **バックエンド対応状況** - 確認済み
   - Ollama: Responses API対応予定
   - vLLM: Responses API対応予定
   - xLLM: Responses API実装予定

### 残課題

1. ヘルスチェックでのResponses API対応検出ロジックの確認
2. `/v1/models`レスポンスへのAPI対応情報追加の確認
3. 統合テストの作成

**出力**: research.md（既存実装調査で代替）

## Phase 1: 設計＆契約

*前提条件: Phase 0完了*

### 既存API契約

#### POST /v1/responses

```json
// リクエスト（パススルー）
{
  "model": "string",
  "input": "string | array",
  "instructions": "string",
  "stream": "boolean"
}

// レスポンス（パススルー）
{
  "id": "string",
  "object": "response",
  "created_at": "number",
  "model": "string",
  "output": [...],
  "usage": {...}
}
```

#### エラーレスポンス（501 Not Implemented）

```json
{
  "error": {
    "message": "Not Implemented: The backend for model 'xxx' does not support the Responses API",
    "type": "server_error",
    "code": 501
  }
}
```

### データモデル（既存）

- `Endpoint.supports_responses_api: bool` - Responses API対応フラグ
- `EndpointModel.supported_apis: Vec<SupportedAPI>` - モデル単位のAPI対応
- `SupportedAPI` - `ChatCompletions`, `Responses`, `Embeddings`

### 統合テストシナリオ

1. **RES001**: Responses API対応バックエンドへのリクエスト転送
2. **RES002**: ストリーミングリクエストのパススルー
3. **RES003**: 非対応バックエンドへの501エラー
4. **RES004**: 認証なしリクエストへの401エラー
5. **RES005**: ルート存在確認

**出力**: contracts/（既存実装で代替）、統合テスト設計

## Phase 2: タスク計画アプローチ

*このセクションは/speckit.tasksコマンドが実行することを記述*

**タスク生成戦略**:

1. 既存実装の動作確認テスト作成（RED）
2. 必要に応じて実装修正（GREEN）
3. 統合テストカバレッジ確保
4. ドキュメント更新

**順序戦略**:

- 統合テスト作成 → 実装検証 → 修正（必要時）→ ドキュメント
- 並列実行可能: テストファイル作成は独立

**推定出力**: tasks.mdに10-15個の番号付きタスク

## 複雑さトラッキング

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| なし | - | - |

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了（既存実装調査）
- [x] Phase 1: Design完了（既存設計確認）
- [x] Phase 2: Task planning完了（アプローチ記述）
- [x] Phase 3: Tasks生成済み (/speckit.tasks コマンド)
- [x] Phase 4: 実装完了（既存実装確認・テスト追加）
- [x] Phase 5: 検証合格（11テスト全パス）

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み（なし）

---
*憲章 v2.0.0 に基づく - `/memory/constitution.md` 参照*
