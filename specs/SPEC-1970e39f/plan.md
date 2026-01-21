# 実装計画: 構造化ロギング強化

**機能ID**: `SPEC-1970e39f` | **日付**: 2025-12-18 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-1970e39f/spec.md`の機能仕様

## 概要

ルーターとノードのHTTPリクエスト/レスポンスを構造化ログとして出力し、
デバッグと監視を容易にする。既存のtracing(Rust)/spdlog(C++)を活用する。

主要な修正点:

1. `openai.rs`にリクエスト受信・ノード選択・エラーのログ追加
2. ノード選択失敗時のリクエスト履歴保存（現在は欠落）
3. ノードポートミスマッチの修正（32768→32769）
4. `HfOnnx`バリアントの追加

## 技術コンテキスト

**言語/バージョン**: Rust 1.75+ (Router), C++17 (Node)
**主要依存関係**: tracing + tracing-subscriber (Rust), spdlog (C++)
**ストレージ**: JSON files (`~/.llm-router/logs/`, `~/.llm-router/request_history.json`)
**テスト**: cargo test, contract tests
**対象プラットフォーム**: macOS, Linux
**プロジェクトタイプ**: single (router + node)
**パフォーマンス目標**: ログ出力によるレイテンシ増加1ms以下
**制約**: 既存ライブラリのみ使用、新規依存追加なし
**スケール/スコープ**: 全OpenAI互換APIエンドポイント

## 憲章チェック

**シンプルさ**: PASS

- プロジェクト数: 2 (router, node) - 既存構造維持
- フレームワーク直接使用: YES (tracing/spdlog)
- 単一データモデル: YES (RequestResponseRecord)
- パターン回避: YES (新規パターン導入なし)

**テスト (妥協不可)**: PASS

- RED-GREEN-Refactorサイクル強制: YES
- テストが実装より先: YES (contract tests first)
- 順序: Contract→Integration→Unit
- 実依存関係使用: YES (実際のファイルシステム)

**可観測性**: PASS - 本機能の主目的

- 構造化ロギング: YES (JSON形式)
- エラーコンテキスト: YES (request_id, endpoint, model, runtime_id)

**バージョニング**: N/A - バグ修正、新バージョン不要

## プロジェクト構造

### ドキュメント

```text
specs/SPEC-1970e39f/
├── spec.md              # 機能仕様 (完了)
├── plan.md              # このファイル
└── tasks.md             # タスク分解 (/speckit.tasksで生成)
```

### 対象ソースコード

```text
router/src/api/openai.rs        # ログ追加、履歴保存修正
router/src/api/proxy.rs         # save_request_record関数
router/src/convert.rs           # ノードポート修正
router/src/registry/models.rs   # HfOnnxバリアント追加
router/tests/contract/          # 新規テスト追加
```

## Phase 0: リサーチ (完了)

### 決定事項

| 項目 | 決定 | 理由 |
|------|------|------|
| Rustロギング | tracing | 既存導入済み、非同期対応、構造化ログ |
| C++ロギング | spdlog | 既存導入済み、高速、JSON対応 |
| ログフォーマット | JSON | jq等での解析可能性 |
| ログローテーション | 日次 | 既存設定維持 |

### 検討した代替案

- **slog (Rust)**: 古い、tracingの方がエコシステム充実
- **plog (C++)**: spdlogの方が高速でJSON対応が良い

## Phase 1: 設計

### データモデル

既存の`RequestResponseRecord`を使用:

```rust
pub struct RequestResponseRecord {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub request_type: RequestType,
    pub model: String,
    pub runtime_id: Uuid,           // Uuid::nil() for no node
    pub node_machine_name: String,
    pub node_ip: IpAddr,
    pub client_ip: Option<IpAddr>,
    pub request_body: Value,
    pub response_body: Option<Value>,
    pub duration_ms: i64,
    pub status: RecordStatus,
    pub completed_at: DateTime<Utc>,
}
```

### API契約 (ログ出力)

| イベント | レベル | 必須フィールド |
|----------|--------|----------------|
| リクエスト受信 | INFO | endpoint, model, request_id |
| ノード選択成功 | INFO | runtime_id, node_ip |
| ノード選択失敗 | ERROR | error, request_id |
| プロキシエラー | WARN | status_code, runtime_id, error |

### 契約テスト設計

```text
router/tests/contract/
├── openai_logging_test.rs      # 新規: ログ出力検証
└── models_source_test.rs       # 新規: HfOnnx検証
```

## Phase 2: タスク計画アプローチ

### タスク生成戦略

1. **Contract tests先行**: ログ出力フォーマット検証テスト
2. **実装**: テストを通すための最小実装
3. **リファクタリング**: 重複コード整理

### 順序戦略

| 優先度 | タスク | 依存 |
|--------|--------|------|
| P0 | Contract test: ログ出力検証 | なし |
| P0 | Contract test: 履歴保存検証 | なし |
| P1 | openai.rs: ログ追加 | P0テスト |
| P1 | openai.rs: 履歴保存修正 | P0テスト |
| P2 | convert.rs: ポート修正 | なし |
| P2 | models.rs: HfOnnx追加 | なし |
| P3 | 品質チェック | 全実装 |

### 推定出力

tasks.mdに10-15個の番号付きタスク

## 複雑さトラッキング

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| なし | - | - |

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了 (アプローチ記述)
- [ ] Phase 3: Tasks生成済み (/speckit.tasks待ち)
- [ ] Phase 4: 実装完了
- [ ] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み (なし)

---
*憲章 v1.0.0 に基づく - `/memory/constitution.md` 参照*
