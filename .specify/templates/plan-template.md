# 実装計画: [機能名]

**機能ID**: `SPEC-[UUID8桁]` | **日付**: [日付] | **仕様**: [リンク]
**入力**: `/specs/SPEC-[UUID8桁]/spec.md` の機能仕様

**注意**: このテンプレートは `/speckit.plan` コマンドによって記入されます。実行ワークフローは `.specify/templates/commands/plan.md` を参照してください。

## 概要

[機能仕様から抽出: 主要要件 + researchからの技術アプローチ]

## 技術コンテキスト

<!--
  要対応: このセクションの内容をプロジェクトの技術詳細に置き換えてください。
  ここでの構成は反復プロセスをガイドするための参考です。
-->

**言語/バージョン**: [例: Python 3.11, Swift 5.9, Rust 1.75 または 要明確化]
**主要依存関係**: [例: FastAPI, UIKit, LLVM または 要明確化]
**ストレージ**: [該当する場合、例: PostgreSQL, CoreData, files または N/A]
**テスト**: [例: pytest, XCTest, cargo test または 要明確化]
**対象プラットフォーム**: [例: Linuxサーバー, iOS 15+, WASM または 要明確化]
**プロジェクトタイプ**: [single/web/mobile - ソース構造を決定]
**パフォーマンス目標**: [ドメイン固有、例: 1000 req/s, 10k lines/sec, 60 fps または 要明確化]
**制約**: [ドメイン固有、例: <200ms p95, <100MB memory, オフライン対応 または 要明確化]
**スケール/スコープ**: [ドメイン固有、例: 10kユーザー, 1M LOC, 50画面 または 要明確化]

## 憲章チェック

*ゲート: Phase 0 research前に合格必須。Phase 1 design後に再チェック。*

[憲章ファイルに基づいてゲートを決定]

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-[UUID8桁]/
├── plan.md              # このファイル (/speckit.plan コマンド出力)
├── research.md          # Phase 0 出力 (/speckit.plan コマンド)
├── data-model.md        # Phase 1 出力 (/speckit.plan コマンド)
├── quickstart.md        # Phase 1 出力 (/speckit.plan コマンド)
├── contracts/           # Phase 1 出力 (/speckit.plan コマンド)
└── tasks.md             # Phase 2 出力 (/speckit.tasks コマンド - /speckit.planでは作成しない)
```

### ソースコード (リポジトリルート)

<!--
  要対応: 以下のプレースホルダーツリーをこの機能の具体的なレイアウトに置き換えてください。
  未使用のオプションを削除し、選択した構造を実際のパス（例: apps/admin, packages/somethingなど）で展開してください。
  最終的な計画にOptionラベルを含めないでください。
-->

```text
# [未使用なら削除] オプション1: 単一プロジェクト (デフォルト)
src/
├── models/
├── services/
├── cli/
└── lib/

tests/
├── contract/
├── integration/
└── unit/

# [未使用なら削除] オプション2: Webアプリケーション ("frontend" + "backend"検出時)
backend/
├── src/
│   ├── models/
│   ├── services/
│   └── api/
└── tests/

frontend/
├── src/
│   ├── components/
│   ├── pages/
│   └── services/
└── tests/

# [未使用なら削除] オプション3: モバイル + API ("iOS/Android"検出時)
api/
└── [上記のbackendと同じ]

ios/ または android/
└── [プラットフォーム固有の構造: 機能モジュール、UIフロー、プラットフォームテスト]
```

**構造決定**: [選択した構造を文書化し、上記でキャプチャした実際のディレクトリを参照]

## 複雑さトラッキング

> **憲章チェックに正当化が必要な違反がある場合のみ記入**

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| [例: 4つ目のプロジェクト] | [現在のニーズ] | [なぜ3つのプロジェクトでは不十分か] |
| [例: Repositoryパターン] | [特定の問題] | [なぜ直接DBアクセスでは不十分か] |
