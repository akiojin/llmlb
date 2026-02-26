# 実装計画: 監査ログ（Audit Log）

**機能ID**: `SPEC-8301d106` | **日付**: 2026-02-20 | **仕様**: [spec.md](spec.md)
**入力**: `specs/SPEC-8301d106/spec.md` の機能仕様

## 概要

llmlbロードバランサーに統一的な監査ログシステムを実装する。
全HTTP操作のメタデータを自動記録し、SHA-256バッチハッシュチェーンによる
改ざん防止、FTS5による全文検索、request\_historyの統合、
アーカイブ管理を提供する。

主要技術アプローチ:

- axumグローバルミドルウェアでリクエスト/レスポンスをキャプチャ
- tokio mpscチャネルによる非同期バッファリング（30秒フラッシュ）
- SQLiteテーブル + FTS5仮想テーブルで検索
- SHA-256バッチハッシュチェーン（5分間隔）で改ざん検知
- request\_historyデータの移行・統合
- React/shadcn/uiで管理者専用ダッシュボードページ

## 技術コンテキスト

**言語/バージョン**: Rust（安定版、既存プロジェクトと同一）+ TypeScript（ダッシュボード）
**主要依存関係**: axum, sqlx, tokio, sha2, serde, chrono, tracing（全て既存）
**ストレージ**: SQLite（メインDB: load\_balancer.db、アーカイブ: audit\_archive.db）
**テスト**: cargo test（Rust）、pnpm test（ダッシュボード）
**対象プラットフォーム**: Linux/macOS（既存と同一）
**プロジェクトタイプ**: Web（Rust API + React SPA）
**パフォーマンス目標**: 監査ログ書き込みによるレイテンシ増加 < 1ms
**制約**: 非同期バッファ上限10,000件、フラッシュ間隔30秒、バッチ間隔5分
**スケール/スコープ**: 90日分 推定100万レコード、構造化検索3秒以内

## 憲章チェック

*ゲート: Phase 0 research前に合格必須。Phase 1 design後に再チェック。*

| 原則 | 状態 | 説明 |
|------|------|------|
| I. Router-Nodeアーキテクチャ | 合格 | Router側のみの変更。Node側への影響なし |
| II. HTTP/REST通信プロトコル | 合格 | 既存のOpenAI互換API仕様に影響なし |
| III. テストファースト | 合格 | TDDサイクル厳守で実装 |
| IV. GPU必須ノード登録要件 | N/A | ノード登録に影響なし |
| V. シンプルさと開発者体験 | 合格 | 既存パターン踏襲、新規crateなし |
| VI. LLM最適化 | 合格 | ページネーション50件/ページ、FTS5検索 |
| VII. 可観測性とロギング | 合格 | 構造化ロギング(tracing)、監査ログ自体が可観測性向上 |
| VIII. 認証・アクセス制御 | 合格 | admin専用アクセス制御、既存認証フロー踏襲 |
| IX. バージョニング | 合格 | Conventional Commits準拠 |

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-8301d106/
├── spec.md              # 機能仕様書
├── plan.md              # この実装計画
├── research.md          # 技術リサーチ
├── data-model.md        # データモデル設計
├── quickstart.md        # セットアップガイド
└── tasks.md             # タスク分解（/speckit.tasksで生成）
```

### ソースコード (リポジトリルート)

```text
llmlb/
├── migrations/
│   └── 017_audit_log.sql              # 監査ログテーブル・FTS5・トリガー
│
├── src/
│   ├── db/
│   │   ├── mod.rs                     # audit_logモジュール追加
│   │   └── audit_log.rs               # AuditLogStorage（CRUD・統計・検証）
│   │
│   ├── audit/
│   │   ├── mod.rs                     # 監査ログモジュール
│   │   ├── middleware.rs              # axum監査ミドルウェア
│   │   ├── writer.rs                  # 非同期バッファ・フラッシュ
│   │   ├── hash_chain.rs             # SHA-256バッチハッシュチェーン
│   │   └── types.rs                   # AuditLogEntry, ActorType等
│   │
│   ├── api/
│   │   ├── mod.rs                     # audit_logルート追加
│   │   └── audit_log.rs              # REST APIハンドラー
│   │
│   ├── lib.rs                         # AppState拡張（audit_log追加）
│   │
│   └── web/dashboard/src/
│       ├── pages/
│       │   └── AuditLog.tsx           # 監査ログページ
│       ├── components/audit/
│       │   ├── AuditLogTable.tsx      # ログテーブル
│       │   ├── AuditLogFilters.tsx    # フィルタ
│       │   └── HashChainStatus.tsx    # 検証UI
│       └── App.tsx                    # ルーティング追加
```

**構造決定**: Web（バックエンドRust + フロントエンドReact）。
既存の`db/`、`api/`、`auth/`パターンに合わせ、
新規モジュール`audit/`をトップレベルに追加。
ダッシュボードは`pages/`と`components/audit/`に分離。

## 実装フェーズ

### Phase 1: 基盤（データモデル・型定義・バッファ）

1. **マイグレーションSQL**: `audit_log_entries`、`audit_batch_hashes`、
   FTS5仮想テーブル、トリガー、インデックス
2. **Rust型定義**: `AuditLogEntry`、`ActorType`、`AuditBatchHash`
3. **AuditLogStorage**: DB CRUD操作（insert\_batch、query、統計）
4. **AuditLogWriter**: tokio mpscチャネル、メモリバッファ、
   30秒フラッシュ、10,000件上限
5. **AppState拡張**: `audit_log_writer`フィールド追加

### Phase 2: ミドルウェア・記録

1. **監査ミドルウェア**: リクエスト/レスポンスのメタデータキャプチャ、
   除外パターンフィルタ、アクター抽出
2. **ハンドラー補足**: 推論ハンドラーからトークン数をExtensionで渡す
3. **認証失敗記録**: 認証ミドルウェアからの失敗情報キャプチャ
4. **ルーター統合**: `create_app`に監査ミドルウェアを追加

### Phase 3: ハッシュチェーン・検証

1. **ハッシュチェーン生成**: バッチフラッシュ時にSHA-256チェーン計算
2. **起動時検証**: サーバー起動時の全バッチ検証
3. **定期検証**: 24時間ごとのバックグラウンド検証タスク
4. **改ざん検出ハンドリング**: アラート出力・新チェーン開始

### Phase 4: request\_history統合

1. **データ移行SQL**: request\_history → audit\_log\_entriesへの移行
2. **トークン統計再実装**: audit\_log\_entriesからの統計クエリ
3. **ダッシュボード統計の切り替え**: 既存のトークン統計APIを
   audit\_log\_entries参照に変更
4. **request\_history廃止**: テーブル・コード・テストの削除

### Phase 5: API・検索

1. **REST APIハンドラー**: 一覧取得、フィルタ検索、検証API、統計API
2. **FTS5検索**: フリーテキスト検索クエリ
3. **ページネーション**: カーソルベースまたはオフセットベース（50件/ページ）
4. **アクセス制御**: admin限定ミドルウェア

### Phase 6: アーカイブ

1. **アーカイブDB管理**: 別SqlitePool、自動作成
2. **アーカイブ処理**: 90日以上のデータ移動
3. **統合検索**: メインDB + アーカイブDBの結果マージ

### Phase 7: ダッシュボードUI

1. **AuditLogページ**: 基本レイアウト・テーブル
2. **フィルタ・検索**: 構造化フィルタ + フリーテキスト
3. **ハッシュチェーン検証UI**: 検証ボタン・結果表示
4. **ルーティング・ナビゲーション**: App.tsx更新

## 複雑さトラッキング

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| 新モジュール`audit/` | 監査ログはDB層・ミドルウェア・バッファ・ハッシュチェーンなど複数の責務を持つため、単一ファイルでは管理困難 | `db/audit_log.rs`のみでは、ミドルウェアやバッファの責務が混在する |
| FTS5仮想テーブル | フルテキスト検索の要件（FR-023）を満たすため。LIKE検索ではパフォーマンス不足 | LIKE '%keyword%'は10万件超で数秒かかり、SC-006（3秒以内）を満たせない |
| 別SqlitePool（アーカイブ） | ATTACH DATABASEはsqlxの接続プールと相性が悪い。別プール方式の方がシンプルでテスト容易 | ATTACH DATABASE方式はコネクション管理が複雑になる |
