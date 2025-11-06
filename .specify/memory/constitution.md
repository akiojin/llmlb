# Unity MCP Server 開発憲章

## 基本原則

### I. ハンドラーベースアーキテクチャ
- すべてのMCPツールはBaseToolHandlerのサブクラスとして実装
- ハンドラーは自己完結型で独立してテスト可能
- 明確な責任分離:
  - **Handler**: 検証、パラメータ処理、オーケストレーション
  - **Tool**: Unity通信の実行
- 新しいツールは必ずhandler/tool両方を実装

### II. Unity通信プロトコル
- すべてのUnity通信はTCP JSON-RPC経由で実行
- コマンドは必ず`workspaceRoot`パラメータを含む
- タイムアウトとリトライ処理は必須:
  - デフォルトタイムアウト: 30秒
  - 指数バックオフによる再接続
  - 最大再接続遅延: 30秒
- エラーレスポンスには実行可能なコンテキストを含む:
  - エラーの原因
  - 推奨される解決策
  - 関連するファイルパス/行番号

### III. テストファースト (妥協不可)
**絶対遵守事項:**
- TDD必須: テスト作成 → テスト失敗(RED) → 実装 → テスト成功(GREEN) → リファクタリング
- Red-Green-Refactorサイクルを厳格に遵守
- Git commitsはテストが実装より先に表示される必要がある
- 順序: Unit → Integration → E2E
- Integration testは実Unity接続を使用 (モック禁止)
- **禁止事項**:
  - テストなしでの実装
  - REDフェーズのスキップ
  - テスト後の実装コミット

**インフラストラクチャコードの例外:**
以下の条件を満たすインフラコードは、厳密なTDDサイクルの代わりに**CI/CD統合テスト**で検証可能:
- **対象**: GitHub Actionsワークフロー、リリーススクリプト、CI/CDパイプライン
- **理由**: ローカル環境でのユニットテストが実質不可能（GitHub Actions環境依存）
- **代替検証**:
  - 品質チェックワークフロー (cargo test, clippy, fmt, commitlint, markdownlint)
  - 実際のPR作成→品質チェック→マージの統合テスト
  - 実リリースフローでの動作確認
- **必須条件**:
  - 複雑さトラッキングに例外を文書化
  - 代替検証方法を明記（plan.md）
  - CI/CDログで検証結果を記録
  - 外部依存が解決次第、実環境で統合テスト実施
- **禁止**:
  - この例外を通常のアプリケーションコードに適用
  - 統合テストなしでの本番デプロイ

### IV. C# LSP統合
- すべてのC# symbol/search/editはバンドルLSP経由で実行
- Unity通信は不要 (LSPは自己完結)
- **禁止**: 行ベース編集 (構造化操作のみ許可)
  - 許可: `script_edit_structured`, `script_refactor_rename`, `script_symbol_find`
  - 禁止: パターン置換、行番号ベース編集
- インデックスベースのシンボル解決を使用
- LSPエラーは詳細なコンテキストと共に返す

### V. シンプルさと開発者体験
**開発者体験の原則:**
- CLI操作は直感的でなければならない
- エラーメッセージは解決策を明示
- LLM最適化されたレスポンス:
  - ページング機能提供
  - 自動要約
  - 最小ペイロード (maxBytes, pageSize使用)
- ドキュメントはREADME/CLAUDE.mdに集約
- 実装はシンプルさを最優先:
  - 複雑な抽象化を避ける
  - YAGNIの原則
  - 必要性が証明されるまで機能追加しない

### VI. LLM最適化
**レスポンス設計:**
- 大きな出力は常にページング可能
- デフォルト制限は控えめに:
  - Search: `pageSize≤20`, `maxBytes≤64KB`
  - Hierarchy: `nameOnly=true`, `maxObjects 100-500`
  - Script read: 30-40行、`maxBytes`設定
- 要約モード提供:
  - エラー≤30件、メッセージ≤200文字
  - 大きなテキスト≤1000文字
- プレビューは診断時のみ (トークン節約)

### VII. 可観測性とロギング
- 構造化ロギング必須
- ログレベル: debug, info, warn, error
- エラーコンテキストは十分に提供:
  - スタックトレース (development mode)
  - パラメータ概要
  - ツール名
- コンソール読み取り機能でUnityログ統合

### VIII. バージョニング
- MAJOR.MINOR.PATCH形式（Semantic Versioning 2.0.0準拠）
- **自動バージョニング（semantic-release）**:
  - Conventional Commitsから自動計算
  - `fix:` → パッチ (+0.0.1)
  - `feat:` → マイナー (+0.1.0)
  - `BREAKING CHANGE:` / `!` → メジャー (+1.0.0)
- **ブランチ別リリース戦略**:
  - `main`: 正式版 (例: v1.2.3)
  - `develop`: プレリリース版 (例: v1.2.3-alpha.1)
  - `hotfix/**`: パッチ版 (mainへ直接マージ)
- **GitHub Actions統合**:
  - semantic-release.ymlが自動実行
  - CHANGELOG.md、Cargo.toml、package.jsonを自動更新
  - GitHub Releaseとバイナリを自動公開
- **手動バージョン指定は禁止**:
  - 手動編集やnpm versionコマンドは使用不可
  - すべてのバージョン管理はsemantic-releaseに委譲

## テスト要件

### カバレッジ目標
- **Unit tests**: 80%以上のコードカバレッジ
- **Integration tests**: すべてのクリティカルパス100%
- **E2E tests**: 主要なユーザーワークフロー100%
- **Performance tests**: レスポンスタイムベンチマーク

### テストカテゴリ
1. **Unit tests** (`tests/unit/`):
   - Handler検証ロジック
   - ユーティリティ関数
   - データ変換

2. **Integration tests** (`tests/integration/`):
   - Unity通信
   - LSP統合
   - ファイルI/O

3. **E2E tests** (`tests/e2e/`):
   - エンドツーエンドワークフロー
   - 実Unity接続必須

4. **Performance tests** (`tests/performance/`):
   - 接続パフォーマンス
   - 大規模データ処理

## ドキュメント要件

### 必須ドキュメント
- `README.md`: プロジェクト概要、セットアップ、使用法 (英語)
- `README.ja.md`: 日本語版README
- `CLAUDE.md`: 開発ワークフロー、ガイドライン (日本語)
- `specs/`: 機能仕様書 (Spec Kit準拠)
  - `SPEC-[UUID8桁]/spec.md`: 機能仕様
  - `SPEC-[UUID8桁]/plan.md`: 実装計画
  - `SPEC-[UUID8桁]/tasks.md`: タスク分解

### ドキュメント原則
- **設計は`docs/`または`specs/`**: README.mdには書かない
- **日本語優先**: 開発ドキュメントは日本語
- **リンク活用**: README.mdは詳細へのリンクのみ
- **Spec Kit準拠**: 新機能は必ず仕様書作成

## CI/CD要件

### 必須チェック
- テストカバレッジ: 80%未満で警告
- リンティング: エラー/警告ゼロ
- commitlint: コミットメッセージ規約準拠
- markdownlint: マークダウンファイル品質

### コミットワークフロー
1. タスク完了
2. テスト実行・合格確認
3. 日本語コミットメッセージ作成
4. `git commit && git push`

## ガバナンス

### 憲章遵守
- 本憲章はすべての開発プラクティスに優先
- すべてのPR/レビューで憲章準拠を確認
- 複雑さは正当化必須 (Complexity Tracking)
- 違反は文書化し、代替案却下理由を記載

### 改定プロセス
- 改定には文書化、承認、移行計画が必要
- バージョン番号でトラッキング
- 変更履歴を保持

**バージョン**: 1.0.0
**制定日**: 2025-10-17
**最終改定**: 2025-10-17
