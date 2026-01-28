# 実装計画: ロードバランサー主導エンドポイント登録システム

**機能ID**: `SPEC-66555000` | **日付**: 2026-01-14 | **仕様**: [spec.md](./spec.md)
**入力**: `specs/SPEC-66555000/spec.md`の機能仕様

## 実行フロー (/speckit.plan コマンドのスコープ)

```text
1. 入力パスから機能仕様を読み込み
   → 完了: SPEC-66555000/spec.md を読み込み済み
2. 技術コンテキストを記入 (要明確化をスキャン)
   → 完了: Rust/SQLite/axum、プロジェクトタイプ single
3. 下記の憲章チェックセクションを評価
   → 完了: TDD必須、シンプルさ優先を確認
4. Phase 0 を実行 → research.md
   → 完了: 技術リサーチ完了
5. Phase 1 を実行 → contracts, data-model.md, quickstart.md
   → 完了: 設計ドキュメント作成
6. 憲章チェックセクションを再評価
   → 完了: 違反なし
7. Phase 2 を計画 → タスク生成アプローチを記述
   → 完了: 下記に記載
8. 停止 - /speckit.tasks コマンドの準備完了
```

## 概要

既存の「ノード自己登録」システムを廃止し、ロードバランサー主導の「エンドポイント」登録方式に統合する。
これにより、自社ノード（マルチエンジン対応: llama.cpp/safetensors.cpp/whisper.cpp等）だけでなく、
Ollama/vLLM等の外部OpenAI互換APIも統一的に管理できるようになる。

**主要変更点**:

- ノード自己登録（POST /v0/nodes）→ ロードバランサー主導登録（POST /v0/endpoints）
- プッシュ型ハートビート（POST /v0/health）→ プル型ヘルスチェック
- GPU必須要件 → 外部エンドポイントはGPU情報不問

### 追加要件（2026-01-26）: エンドポイントタイプ自動判別

エンドポイント登録時にサーバータイプ（xLLM/Ollama/vLLM/OpenAI互換）を自動判別し、
タイプに応じた機能制御を行う。

**追加変更点**:

- エンドポイント登録時にタイプを自動判別・永続化
- タイプに基づくAPIフィルタリング機能
- xLLMエンドポイント専用：モデルダウンロード機能
- xLLM/Ollamaエンドポイント：モデルメタデータ（最大トークン数）取得

## 技術コンテキスト

**言語/バージョン**: Rust 1.75+, TypeScript 5.x (ダッシュボード)
**主要依存関係**: axum 0.7, sqlx 0.7, tokio 1.x, reqwest 0.11
**ストレージ**: SQLite (lb.db) - sqlx migrations
**テスト**: cargo test, playwright (E2E)
**対象プラットフォーム**: Windows / macOS / Linux
**プロジェクトタイプ**: single (router + node)
**パフォーマンス目標**: エンドポイント登録 < 100ms、ヘルスチェック < 1s
**制約**: 既存クラウドプレフィックス（openai:, google:, anthropic:）維持
**スケール/スコープ**: 最大100エンドポイント、1000モデル

## 憲章チェック

*ゲート: Phase 0 research前に合格必須。Phase 1 design後に再チェック。*

**シンプルさ**:

- プロジェクト数: 1 (router) ✅
- フレームワークを直接使用? ✅ axum/sqlxを直接利用
- 単一データモデル? ✅ Endpoint構造体のみ
- パターン回避? ✅ Repository/UoWなし

**アーキテクチャ**:

- すべての機能をライブラリとして? ✅ llmlb/src/以下にモジュール化
- ライブラリリスト:
  - `db/endpoints.rs`: エンドポイントDB操作
  - `registry/endpoints.rs`: エンドポイントレジストリ
  - `api/endpoints.rs`: REST APIハンドラー
  - `health/endpoint_checker.rs`: ヘルスチェッカー
- ライブラリごとのCLI: N/A (APIサーバー)
- ライブラリドキュメント: N/A

**テスト (妥協不可)**:

- RED-GREEN-Refactorサイクルを強制? ✅ 必須
- Gitコミットはテストが実装より先に表示? ✅ 必須
- 順序: Contract→Integration→E2E→Unitを厳密に遵守? ✅
- 実依存関係を使用? ✅ SQLite in-memory
- Integration testの対象: 新しいライブラリ、契約変更、共有スキーマ? ✅
- 禁止: テスト前の実装、REDフェーズのスキップ ✅

**可観測性**:

- 構造化ロギング含む? ✅ tracing
- フロントエンドログ → バックエンド? N/A
- エラーコンテキスト十分? ✅

**バージョニング**:

- バージョン番号割り当て済み? ✅ semantic-release
- 変更ごとにBUILDインクリメント? ✅ 自動
- 破壊的変更を処理? ✅ 移行計画あり

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-66555000/
├── plan.md              # このファイル (/speckit.plan コマンド出力)
├── research.md          # Phase 0 出力 (/speckit.plan コマンド)
├── data-model.md        # Phase 1 出力 (/speckit.plan コマンド)
├── quickstart.md        # Phase 1 出力 (/speckit.plan コマンド)
├── contracts/           # Phase 1 出力 (/speckit.plan コマンド)
└── tasks.md             # Phase 2 出力 (/speckit.tasks コマンド)
```

### ソースコード (リポジトリルート)

```text
# 単一プロジェクト構造
llmlb/
├── src/
│   ├── api/
│   │   ├── endpoints.rs     # 新規: エンドポイントCRUD API
│   │   └── nodes.rs         # 削除対象: 旧ノードAPI
│   ├── db/
│   │   ├── endpoints.rs     # 新規: エンドポイントDB層
│   │   └── nodes.rs         # 削除対象: 旧ノードDB層
│   ├── registry/
│   │   ├── endpoints.rs     # 新規: エンドポイントレジストリ
│   │   └── mod.rs           # 変更: レジストリ統合
│   └── health/
│       └── endpoint_checker.rs  # 新規: プル型ヘルスチェック
├── migrations/
│   └── YYYYMMDDHHMMSS_add_endpoints.sql  # 新規: スキーマ
└── tests/
    ├── contract/
    │   └── endpoints_api_test.rs  # 新規
    └── integration/
        └── endpoint_lifecycle_test.rs  # 新規
```

**構造決定**: 単一プロジェクト（オプション1）

## Phase 0: アウトライン＆リサーチ

### 技術コンテキストの不明点

1. **Ollama API仕様**: GET /api/tags でモデル一覧取得
2. **vLLM API仕様**: GET /v1/models でモデル一覧取得
3. **OpenAI互換API仕様**: GET /v1/models でモデル一覧取得
4. **自社ノードAPI**: GET /v1/models でモデル一覧取得（既存）

### リサーチ結果

詳細は [research.md](./research.md) を参照。

**決定事項**:

- エンドポイント: すべてOpenAI互換APIとして統一的に扱う
- ヘルスチェック方法: GET /v1/models（統一）
- モデル同期: GET /v1/models（統一）

**出力**: すべての要明確化が解決されたresearch.md

### 追加リサーチ（2026-01-26）: エンドポイントタイプ判別

**タイプ判別方法**:

| タイプ | 判別方法 | 根拠 |
|--------|----------|------|
| xLLM | `GET /v0/system` レスポンスに `xllm_version` | 本プロジェクト独自エンドポイント |
| Ollama | `GET /api/tags` が有効、または `Ollama` ヘッダー | Ollama標準API |
| vLLM | `GET /v1/models` + `vllm` in Server header | vLLM標準動作 |
| OpenAI互換 | 上記いずれにも該当しない | フォールバック |

**判別優先順位**: xLLM > Ollama > vLLM > OpenAI互換

**タイプ固有機能**:

| 機能 | xLLM | Ollama | vLLM | OpenAI互換 |
|------|------|--------|------|-----------|
| モデルダウンロード | ✅ | ❌ | ❌ | ❌ |
| 最大トークン数取得 | ✅ | ✅ | ❌ | ❌ |
| モデル一覧同期 | ✅ | ✅ | ✅ | ✅ |
| ヘルスチェック | ✅ | ✅ | ✅ | ✅ |

**xLLMモデルダウンロードAPI**:

```
POST /v0/endpoints/:id/download
{
  "model": "Qwen/Qwen2.5-7B-Instruct-GGUF",
  "filename": "qwen2.5-7b-instruct-q4_k_m.gguf"
}

GET /v0/endpoints/:id/download/progress
{
  "model": "...",
  "progress": 45.2,
  "speed_mbps": 120.5,
  "eta_seconds": 180
}
```

**モデルメタデータ取得**:

- xLLM: `GET /v0/models/:model/info` → `context_length`
- Ollama: `POST /api/show` → `parameters.num_ctx`

## Phase 1: 設計＆契約

*前提条件: research.md完了*

### 1. データモデル

詳細は [data-model.md](./data-model.md) を参照。

**主要エンティティ**:

- `Endpoint`: エンドポイント情報（name, url, status, api_key, **endpoint_type**等）
- `EndpointModel`: エンドポイントで利用可能なモデル情報（**max_tokens**追加）
- `EndpointStatus`: pending, online, offline, error
- `EndpointType`: xllm, ollama, vllm, openai_compatible, unknown（追加）
- `ModelDownloadTask`: モデルダウンロードタスク（xLLM専用、追加）

### 2. API契約

詳細は [contracts/](./contracts/) を参照。

```text
# エンドポイント管理API（認証必須）
POST   /v0/endpoints              # 登録（タイプ自動判別）
GET    /v0/endpoints              # 一覧（?type=xllm でフィルタ可能）
GET    /v0/endpoints/:id          # 詳細（タイプ情報含む）
PUT    /v0/endpoints/:id          # 更新（タイプ手動変更可能）
DELETE /v0/endpoints/:id          # 削除
POST   /v0/endpoints/:id/test     # 接続テスト（タイプ再判別）
POST   /v0/endpoints/:id/sync     # モデル同期

# xLLM専用API（タイプ=xllmのみ許可）
POST   /v0/endpoints/:id/download          # モデルダウンロード開始
GET    /v0/endpoints/:id/download/progress # ダウンロード進捗

# モデルメタデータAPI（xLLM/Ollamaのみ）
GET    /v0/endpoints/:id/models/:model/info  # モデル情報（max_tokens等）
```

### 3. 契約テスト

- 各エンドポイントのリクエスト/レスポンススキーマ検証
- 認証要件の検証
- エラーレスポンス形式の検証

### 4. テストシナリオ

- エンドポイント登録 → ヘルスチェック → オンライン遷移
- エンドポイント停止 → オフライン検知
- モデル同期 → モデル一覧更新
- エンドポイント登録 → タイプ自動判別（xLLM/Ollama/vLLM/OpenAI互換）
- タイプフィルタリング → 指定タイプのみ取得
- xLLMへモデルダウンロード → 進捗取得 → 完了確認
- 非xLLMへモデルダウンロード → エラー（サポート外）
- xLLM/Ollamaからモデルメタデータ取得 → max_tokens確認

### 5. クイックスタート

詳細は [quickstart.md](./quickstart.md) を参照。

**出力**: data-model.md, /contracts/*, quickstart.md

## Phase 2: タスク計画アプローチ

*このセクションは/speckit.tasksコマンドが実行することを記述 - /speckit.plan中は実行しない*

**タスク生成戦略**:

- `/templates/tasks-template.md` をベースとして読み込み
- Phase 1設計ドキュメント (contracts, data model, quickstart) からタスクを生成
- 各contract → contract testタスク [P]
- 各entity → model作成タスク [P]
- 各ユーザーストーリー → integration testタスク
- テストを合格させる実装タスク

**順序戦略**:

1. **Setup**: マイグレーション、基本構造
2. **Test (RED)**: 契約テスト、統合テスト（失敗する状態）
3. **Core (GREEN)**: DB層、レジストリ、APIハンドラー
4. **Integration**: ヘルスチェッカー、モデル同期
5. **Polish**: ダッシュボード統合、ドキュメント

**追加タスク（エンドポイントタイプ機能）**:

- **Type Detection**: タイプ判別ロジック実装
- **xLLM Integration**: モデルダウンロードAPI
- **Metadata**: モデルメタデータ取得API
- **Dashboard**: タイプ表示UI

**並列実行マーク**:

- [P] 独立したファイル作成タスク
- [P] 独立したテスト作成タスク

**推定タスク数**: 35-40個（追加要件含む）

**重要**: このフェーズは/speckit.tasksコマンドで実行、/speckit.planではない

## 複雑さトラッキング

*憲章チェックに正当化が必要な違反がある場合のみ記入*

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| なし | - | - |

## 進捗トラッキング

*このチェックリストは実行フロー中に更新される*

**フェーズステータス**:

- [x] Phase 0: Research完了 (/speckit.plan コマンド)
- [x] Phase 1: Design完了 (/speckit.plan コマンド)
- [x] Phase 2: Task planning完了 (/speckit.plan コマンド - アプローチのみ記述)
- [x] Phase 3: Tasks生成済み (/speckit.tasks コマンド)
- [x] Phase 4: 実装完了
- [x] Phase 5: 検証合格

**追加要件ステータス（2026-01-26）**:

- [x] エンドポイントタイプ判別リサーチ完了
- [x] タイプ固有機能設計完了
- [x] タイプ判別テスト作成
- [x] タイプ判別実装
- [x] xLLMモデルダウンロードテスト作成
- [x] xLLMモデルダウンロード実装
- [x] モデルメタデータ取得テスト作成
- [x] モデルメタデータ取得実装
- [x] ダッシュボードタイプ表示

**追加要件ステータス（2026-01-26）**:

- [x] エンドポイントタイプ判別リサーチ完了
- [x] タイプ固有機能設計完了
- [ ] タイプ判別テスト作成
- [ ] タイプ判別実装
- [ ] xLLMモデルダウンロードテスト作成
- [ ] xLLMモデルダウンロード実装
- [ ] モデルメタデータ取得テスト作成
- [ ] モデルメタデータ取得実装
- [ ] ダッシュボードタイプ表示

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み

---

*憲章 v2.0.0 に基づく - `/memory/constitution.md` 参照*
