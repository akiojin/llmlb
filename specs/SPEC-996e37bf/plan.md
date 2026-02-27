# 実装計画: クラウドプロバイダーモデル一覧統合

**機能ID**: `SPEC-996e37bf` | **日付**: 2025-12-25 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-996e37bf/spec.md`の機能仕様

## 概要

`GET /v1/models` エンドポイントを拡張し、ローカルモデルに加えて
OpenAI/Google/Anthropicのクラウドプロバイダーからモデル一覧を取得・マージして返却する。
24時間TTLのキャッシュで効率化し、プロバイダー障害時もフォールバックで継続動作する。

## 技術コンテキスト

**言語/バージョン**: Rust 1.75+
**主要依存関係**: axum, reqwest, tokio, serde_json, chrono
**ストレージ**: インメモリキャッシュ（`tokio::sync::RwLock`）
**テスト**: cargo test, wiremock（モックサーバー）
**対象プラットフォーム**: Linux/macOS サーバー
**プロジェクトタイプ**: single (既存routerプロジェクトへの追加)
**パフォーマンス目標**: キャッシュヒット時100ms以内
**制約**: タイムアウト10秒、APIレート制限遵守
**スケール/スコープ**: 3プロバイダー、各プロバイダー100-500モデル程度

## 憲章チェック

*ゲート: Phase 0 research前に合格必須。Phase 1 design後に再チェック。*

**シンプルさ**:

- プロジェクト数: 1 (既存router) ✅
- フレームワークを直接使用? ✅ axum/reqwestを直接使用
- 単一データモデル? ✅ CloudModelInfo一つで対応
- パターン回避? ✅ シンプルなモジュール追加のみ

**アーキテクチャ**:

- すべての機能をライブラリとして? ✅ cloud_models.rsモジュールとして追加
- ライブラリリスト: cloud_models（クラウドモデル取得ロジック）
- ライブラリごとのCLI: N/A（API機能）
- ライブラリドキュメント: N/A

**テスト (妥協不可)**:

- RED-GREEN-Refactorサイクルを強制? ✅
- Gitコミットはテストが実装より先に表示? ✅
- 順序: Contract→Integration→E2E→Unitを厳密に遵守? ✅
- 実依存関係を使用? wiremockでモック（外部API依存のため例外）
- Integration testの対象: cloud_models.rs, openai.rs変更
- 禁止: テスト前の実装、REDフェーズのスキップ

**可観測性**:

- 構造化ロギング含む? ✅ tracing使用
- エラーコンテキスト十分? ✅ プロバイダー名、エラー理由を含む

**バージョニング**:

- バージョン番号割り当て済み? ✅ SemVerリリースフロー
- 破壊的変更を処理? N/A（後方互換）

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-996e37bf/
├── spec.md              # 機能仕様 (完了)
├── plan.md              # このファイル
├── research.md          # Phase 0 出力
├── data-model.md        # Phase 1 出力
├── quickstart.md        # Phase 1 出力
├── contracts/           # Phase 1 出力
└── tasks.md             # Phase 2 出力 (/speckit.tasks)
```

### ソースコード変更

```text
llmlb/src/api/
├── mod.rs               # pub mod cloud_models; 追加
├── cloud_models.rs      # 新規: クラウドモデル取得ロジック
└── openai.rs            # list_models() 拡張

llmlb/src/
└── lib.rs               # AppStateにキャッシュ追加（必要に応じて）
```

## Phase 0: アウトライン＆リサーチ

### 技術コンテキストからの不明点

1. **各プロバイダーのモデル一覧API形式**:
   - OpenAI: `GET /v1/models` → `{data: [{id, object, created, owned_by}]}`
   - Google: `GET /v1beta/models?key=API_KEY` → `{models: [{name, displayName}]}`
   - Anthropic: `GET /v1/models` → `{data: [{id, type, display_name, created_at}]}`

2. **キャッシュ実装パターン**:
   - 既存の `GGUF_DISCOVERY_CACHE` パターンを参考
   - `OnceCell<RwLock<CacheEntry>>` で遅延初期化

3. **並列API呼び出し**:
   - `futures::join_all` で3プロバイダー同時呼び出し
   - 個別タイムアウト（10秒）で障害隔離

### リサーチ結果

**出力**: research.md で詳細を文書化

## Phase 1: 設計＆契約

*前提条件: research.md完了*

### データモデル

**CloudModelInfo**:

- `id`: String - プレフィックス付きモデルID（例: `openai:gpt-4o`）
- `object`: String - 固定値 `"model"`
- `created`: i64 - 作成日時（Unixタイムスタンプ）
- `owned_by`: String - プロバイダー名

**CloudModelsCache**:

- `models`: `Vec<CloudModelInfo>` - キャッシュされたモデル一覧
- `fetched_at`: `chrono::DateTime<Utc>` - 取得時刻
- `ttl`: `Duration` - 有効期限（24時間）

### API契約

既存の `/v1/models` レスポンスを拡張:

```json
{
  "object": "list",
  "data": [
    {"id": "local-model", "owned_by": "lb", ...},
    {"id": "openai:gpt-4o", "owned_by": "openai", ...},
    {"id": "google:gemini-2.0-flash", "owned_by": "google", ...},
    {"id": "anthropic:claude-sonnet-4-20250514", "owned_by": "anthropic", ...}
  ]
}
```

**出力**: data-model.md, contracts/, quickstart.md

## Phase 2: タスク計画アプローチ

*このセクションは/speckit.tasksコマンドが実行することを記述*

**タスク生成戦略**:

- TDDサイクル: テスト作成 → 失敗確認 → 実装 → 成功確認
- モジュール単位: cloud_models.rs新規作成 → openai.rs拡張
- プロバイダー単位: OpenAI → Google → Anthropic

**順序戦略**:

1. Setup: cloud_models.rsスケルトン作成
2. Test: 各プロバイダーのパーステスト作成
3. Core: fetch関数実装
4. Integration: list_models()拡張
5. Polish: ドキュメント更新

**推定出力**: 15-20個のタスク

## 複雑さトラッキング

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| wiremock使用 | 外部APIテストにモック必須 | 実APIは不安定・コスト発生 |

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了
- [x] Phase 3: Tasks生成済み
- [ ] Phase 4: 実装完了
- [ ] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み

---

*憲章 v1.0.0 に基づく - `/memory/constitution.md` 参照*
