# 実装計画: llmlb CLIコマンド

**機能ID**: `SPEC-669176b2` | **日付**: 2026-01-08 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-669176b2/spec.md`の機能仕様

## 実行フロー (/speckit.plan コマンドのスコープ)

```
1. 入力パスから機能仕様を読み込み ✓
2. 技術コンテキストを記入 ✓
3. 憲章チェックセクションを評価 ✓
4. Phase 0 を実行 → research.md ✓
5. Phase 1 を実行 → contracts, data-model.md, quickstart.md ✓
6. 憲章チェックセクションを再評価 ✓
7. Phase 2 を計画 → タスク生成アプローチを記述 ✓
8. 停止 - /speckit.tasks コマンドの準備完了 ✓
```

## 概要

ollama互換のCLIコマンドをllmlbに実装。ノード単体モードとロードバランサーモードの両方で操作可能にする。
主要機能: `serve`, `run`, `pull`, `list`, `show`, `rm`, `stop`, `ps` (nodeサブコマンド)、
`nodes`, `models`, `status` (routerサブコマンド)。

## 技術コンテキスト

**言語/バージョン**: C++17 (node), Rust 2021 edition (router)
**主要依存関係**: httplib.h, nlohmann/json, llama.cpp
**ストレージ**: ファイルシステム (`~/.llmlb/models/`)
**テスト**: CTest (node), cargo test (router)
**対象プラットフォーム**: macOS (Metal), Linux (CUDA)
**プロジェクトタイプ**: single (node/routerは既存構造を維持)
**パフォーマンス目標**: `list`/`ps`/`show`は1秒以内、REPL応答開始はollama同等(±10%)
**制約**: GPUが必須、サーバー起動が必要
**スケール/スコープ**: 単一ノード〜複数ノードクラスタ

## 憲章チェック

*ゲート: Phase 0 research前に合格必須。Phase 1 design後に再チェック。*

**シンプルさ**:

- プロジェクト数: 2 (node, router) - 既存構造維持 ✓
- フレームワークを直接使用? ✓ (httplib.h, llama.cpp直接利用)
- 単一データモデル? ✓ (ModelInfo, Node, Session)
- パターン回避? ✓ (シンプルなサブコマンドパーサー)

**アーキテクチャ**:

- すべての機能をライブラリとして? ✓ (node/src/cli/, node/src/api/)
- ライブラリリスト:
  - `cli_parser`: サブコマンド解析
  - `cli_client`: サーバー接続クライアント
  - `repl_session`: REPL対話管理
  - `ollama_compat`: ollamaモデル参照
  - `progress_renderer`: プログレス表示
- ライブラリごとのCLI: `llmlb --help`, `llmlb node --help`
- ライブラリドキュメント: quickstart.md形式で提供 ✓

**テスト (妥協不可)**:

- RED-GREEN-Refactorサイクルを強制? ✓
- Gitコミットはテストが実装より先に表示? ✓
- 順序: Contract→Integration→E2E→Unit? ✓
- 実依存関係を使用? ✓ (実サーバー、実ファイルシステム)
- Integration testの対象: CLI解析、サーバー通信、プログレス表示
- 禁止: テスト前の実装、REDフェーズのスキップ ✓

**可観測性**:

- 構造化ロギング含む? ✓ (spdlog使用)
- エラーコンテキスト十分? ✓ (終了コード、エラーメッセージ)

**バージョニング**:

- バージョン番号割り当て済み? ✓ (semantic-release準拠)
- 変更ごとにBUILDインクリメント? ✓
- 破壊的変更を処理? N/A (新機能追加)

## プロジェクト構造

### ドキュメント (この機能)

```
specs/SPEC-669176b2/
├── plan.md              # このファイル
├── research.md          # Phase 0 出力 ✓
├── data-model.md        # Phase 1 出力 ✓
├── quickstart.md        # Phase 1 出力 ✓
├── contracts/           # Phase 1 出力 ✓
│   └── cli-commands.md  # CLIコマンド契約
└── tasks.md             # Phase 2 出力 (/speckit.tasks)
```

### ソースコード (リポジトリルート)

```
node/
├── include/
│   └── utils/
│       └── cli.h           # 拡張: サブコマンド対応
├── src/
│   ├── utils/
│   │   └── cli.cpp         # 拡張: サブコマンドパーサー
│   ├── cli/                # 新規: CLIコマンド実装
│   │   ├── cli_client.h
│   │   ├── cli_client.cpp
│   │   ├── commands/
│   │   │   ├── serve.cpp
│   │   │   ├── run.cpp
│   │   │   ├── pull.cpp
│   │   │   ├── list.cpp
│   │   │   ├── show.cpp
│   │   │   ├── rm.cpp
│   │   │   ├── stop.cpp
│   │   │   └── ps.cpp
│   │   ├── repl_session.h
│   │   ├── repl_session.cpp
│   │   ├── progress_renderer.h
│   │   └── progress_renderer.cpp
│   └── main.cpp            # 拡張: サブコマンド分岐
└── tests/
    ├── contract/
    │   └── cli_contract_test.cpp
    ├── integration/
    │   └── cli_integration_test.cpp
    └── unit/
        └── cli_unit_test.cpp
```

**構造決定**: オプション1 (単一プロジェクト) - nodeの既存構造を拡張

## Phase 0: アウトライン＆リサーチ

**完了** - [research.md](./research.md) 参照

主要決定事項:

1. 既存CLI (`node/src/utils/cli.cpp`) を拡張してサブコマンド対応
2. ollamaスタイルのサーバー常駐＋クライアント接続方式
3. 既存の `ModelSync`/`ModelDownloader` を活用
4. readline互換のシンプルなREPL実装
5. `~/.ollama/models/` のmanifest解析で読み取り専用参照

## Phase 1: 設計＆契約

**完了**

1. **データモデル** → [data-model.md](./data-model.md)
   - Model, Node, Session, Message, DownloadProgress, OllamaModel

2. **API契約** → [contracts/cli-commands.md](./contracts/cli-commands.md)
   - 11個のCLIコマンド仕様
   - 終了コード定義
   - 環境変数定義

3. **クイックスタート** → [quickstart.md](./quickstart.md)
   - 基本操作ガイド
   - トラブルシューティング
   - テスト検証シナリオ

## Phase 2: タスク計画アプローチ

*このセクションは/speckit.tasksコマンドが実行することを記述*

**タスク生成戦略**:

- `/templates/tasks-template.md` をベースとして読み込み
- Phase 1設計ドキュメント (contracts, data model, quickstart) からタスクを生成
- 各CLIコマンド → contract testタスク + 実装タスク
- 各エンティティ → model作成タスク
- 各ユーザーストーリー → integration testタスク

**順序戦略**:

1. Setup: ディレクトリ構造、依存関係
2. Test (RED): CLIパーサーテスト、サーバー通信テスト
3. Core: サブコマンドパーサー、CLIClient、各コマンド実装
4. Integration: エンドツーエンドテスト
5. Polish: ドキュメント、エラーメッセージ

**並列実行マーク [P]**:

- 各コマンド実装は独立しているため並列可能
- serve/run は依存関係あり（serve が先）
- テストは実装前に作成（TDD）

**推定出力**: tasks.mdに約30個のタスク

## 複雑さトラッキング

*憲章チェックに正当化が必要な違反はなし*

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| なし | - | - |

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了 (アプローチのみ記述)
- [x] Phase 3: Tasks生成済み (/speckit.tasks コマンド) - 49タスク
- [x] Phase 4: 実装完了
- [x] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み (なし)

---

*憲章 v2.0.0 に基づく - `/memory/constitution.md` 参照*
