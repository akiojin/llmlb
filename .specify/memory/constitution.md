<!--
Sync Impact Report:
Version change: 1.0.0 → 2.0.0 (MAJOR - project name and architecture redefinition)

Modified principles:
- I. ハンドラーベースアーキテクチャ → Router-Nodeアーキテクチャ
- II. Unity通信プロトコル → HTTP/REST通信プロトコル
- IV. C# LSP統合 → 削除（LLM Routerには不要）

Added sections:
- GPU必須ノード登録要件
- OpenAI互換API
- 認証・アクセス制御

Removed sections:
- Unity通信プロトコル（Unityプロジェクトではない）
- C# LSP統合（Rust/C++プロジェクト）

Templates requiring updates:
- ✅ plan-template.md: 憲章参照を v2.0.0 に更新必要
- ✅ spec-template.md: 変更不要
- ✅ tasks-template.md: 変更不要

Follow-up TODOs: なし
-->

# LLM Router 開発憲章

## 基本原則

### I. Router-Nodeアーキテクチャ

- **Router (Rust)**: 統一APIエンドポイント、ロードバランシング、認証
- **Node (C++)**: llama.cpp/GPT-OSS/Whisper/Stable Diffusion による推論実行
- 明確な責任分離:
  - **Router**: リクエスト検証、ノード選択、レスポンス集約
  - **Node**: モデル読み込み、推論実行、結果返却
- ノードは自己登録方式（Router主導のプッシュ配布禁止）

### II. HTTP/REST通信プロトコル

- すべてのRouter-Node通信はHTTP/JSON経由で実行
- OpenAI互換APIを提供:
  - `/v1/chat/completions` - チャット補完
  - `/v1/models` - モデル一覧
  - `/v1/audio/speech` - TTS
  - `/v1/audio/transcriptions` - ASR
  - `/v1/images/generations` - 画像生成
- タイムアウトとリトライ処理は必須:
  - デフォルトタイムアウト: 120秒（推論）
  - ヘルスチェック間隔: 30秒
  - 失敗ノードの自動除外
- エラーレスポンスにはOpenAI互換形式を使用

### III. テストファースト (妥協不可)

**絶対遵守事項:**

- TDD必須: テスト作成 → テスト失敗(RED) → 実装 → テスト成功(GREEN) → リファクタリング
- Red-Green-Refactorサイクルを厳格に遵守
- Git commitsはテストが実装より先に表示される必要がある
- 順序: Contract → Integration → E2E → Unit
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
- **禁止**:
  - この例外を通常のアプリケーションコードに適用
  - 統合テストなしでの本番デプロイ

### IV. GPU必須ノード登録要件

- GPU未搭載ノードの登録は禁止
- ノード登録時にGPUデバイス情報を必須とする
- API・UI・テストでGPU検証を実装
- CPU-onlyノードはエラーで拒否

### V. シンプルさと開発者体験

**開発者体験の原則:**

- CLI操作は直感的でなければならない
- エラーメッセージは解決策を明示
- ドキュメントはREADME/CLAUDE.mdに集約
- 実装はシンプルさを最優先:
  - 複雑な抽象化を避ける
  - YAGNIの原則
  - 必要性が証明されるまで機能追加しない
- 要件を満たすOSSライブラリが存在する場合は、車輪の再発明を避け優先的に採用

### VI. LLM最適化

**レスポンス設計:**

- 大きな出力は常にページング可能
- デフォルト制限は控えめに:
  - リクエスト履歴: ページサイズ≤50
  - モデル一覧: 全モデル返却（キャッシュ付き）
- ストリーミングレスポンス対応（stream=true）
- トークン節約のための最小ペイロード

### VII. 可観測性とロギング

- 構造化ロギング必須（tracing/slog使用）
- ログレベル: debug, info, warn, error
- エラーコンテキストは十分に提供:
  - リクエストID
  - モデル名・ノード情報
  - エラー詳細とスタックトレース
- リクエスト履歴の保存（7日間保持）

### VIII. 認証・アクセス制御

- **JWT認証（ダッシュボード）**: ユーザーログイン必須
- **APIキー認証（OpenAI互換API）**: Bearer token形式
- 開発モードでも認証フローをスキップしない:
  - デバッグ用認証情報は `#[cfg(debug_assertions)]` で保護
  - リリースビルドでは無効化
- 認証チェックをスキップする環境変数やフラグは禁止

### IX. バージョニング

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
- **手動バージョン指定は禁止**:
  - すべてのバージョン管理はsemantic-releaseに委譲

## テスト要件

### カバレッジ目標

- **Contract tests**: API契約の100%カバー
- **Integration tests**: すべてのクリティカルパス100%
- **E2E tests**: 主要なユーザーワークフロー
- **Unit tests**: 80%以上のコードカバレッジ

### テストカテゴリ

1. **Contract tests** (`llmlb/tests/contract/`):
   - OpenAI互換APIスキーマ検証
   - リクエスト/レスポンス形式

2. **Integration tests** (`llmlb/tests/integration/`):
   - Router-Node通信
   - データベース操作
   - 認証フロー

3. **E2E tests** (`llmlb/tests/e2e/`):
   - エンドツーエンドワークフロー
   - ダッシュボード操作

4. **Unit tests** (`llmlb/src/**/tests.rs`, `xllm/tests/`):
   - 個別モジュール検証
   - ユーティリティ関数

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

- `cargo fmt --check`: フォーマット
- `cargo clippy -- -D warnings`: リンティング
- `cargo test`: テスト実行
- `commitlint`: コミットメッセージ規約準拠
- `markdownlint`: マークダウンファイル品質

### コミットワークフロー

1. タスク完了
2. `make quality-checks` 実行・合格確認
3. Conventional Commitsに従ったコミットメッセージ作成
4. `git commit && git push`

## ガバナンス

### 憲章遵守

- 本憲章はすべての開発プラクティスに優先
- すべてのPR/レビューで憲章準拠を確認
- 複雑さは正当化必須 (Complexity Tracking)
- 違反は文書化し、代替案却下理由を記載

### 改定プロセス

- 改定には文書化、承認、移行計画が必要
- バージョン番号でトラッキング（Semantic Versioning）:
  - MAJOR: 原則の削除・再定義
  - MINOR: 新原則・セクション追加
  - PATCH: 明確化・誤字修正
- 変更履歴を保持

**バージョン**: 2.0.0
**制定日**: 2025-10-17
**最終改定**: 2025-12-26
