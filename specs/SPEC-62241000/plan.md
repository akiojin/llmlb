# 実装計画: 包括的E2Eテストスイート強化

**機能ID**: `SPEC-62241000` | **日付**: 2026-02-13 | **仕様**: `specs/SPEC-62241000/spec.md`
**入力**: `/specs/SPEC-62241000/spec.md` の機能仕様

## 概要

現在68テスト（61パス/7スキップ）のPlaywright E2Eテストスイートを200以上に拡張する。
テストカバレッジが皆無だったUI機能（Statistics/History/Logs/User Management/Endpoint編集/
ログアウト/モバイル/Playground設定/cURL/更新バナー）、API（Audio/Image/Responses/SSE/
権限マトリクス/APIキーライフサイクル/Prometheus）、インテグレーション（LBロードバランシング/
エンドポイントタイプ検出/モデルダウンロード/大規模負荷テスト/データビジュアライゼーション）を
網羅的にカバーする。

技術アプローチ:

- 既存の`mock-openai-endpoint.ts`を拡張し、Audio/Image/Responses APIのモックハンドラーを追加
- エンドポイントタイプ別モック（xLLM/Ollama/vLLM応答パターン）を新規作成
- 既存Page Objectパターン（auth.page.ts/dashboard.page.ts/playground.page.ts）を拡張
- テストは既存ディレクトリ構造（specs/auth, specs/dashboard, specs/api, specs/workflows）に追加

## 技術コンテキスト

**言語/バージョン**: TypeScript 5.x (Playwright テスト)、テスト対象は Rust (llmlb)
**主要依存関係**: Playwright Test、Node.js (mock server)、pnpm
**ストレージ**: N/A（テストコードのみ）
**テスト**: Playwright Test (`npx playwright test`)
**対象プラットフォーム**: Chromium（playwright.config.ts設定済み）
**プロジェクトタイプ**: web（既存E2Eテストプロジェクトへの追加）
**パフォーマンス目標**: 全テスト完了が5分以内（headed mode）
**制約**: 外部依存ゼロ（全モックサーバー使用）、既存68テスト破壊禁止
**スケール/スコープ**: 200+テスト、19ユーザーストーリー、27機能要件

## 憲章チェック

*ゲート: Phase 0 research前に合格必須。Phase 1 design後に再チェック。*

| 項目 | 判定 | 根拠 |
|------|------|------|
| TDD必須 | **合格** | E2Eテスト自体がテストコードであり、RED→GREEN→REFACTORサイクルに適合 |
| シンプルなアーキテクチャ | **合格** | 既存のPage Object + helper + mock patternを拡張するのみ |
| 既存コード破壊禁止 | **合格** | 新規テストファイル追加が主体、既存ファイルは拡張のみ |
| 外部依存ゼロ | **合格** | 全テストはmock serverで動作し、xLLM/Ollama/vLLM等の実サーバー不要 |

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-62241000/
├── plan.md              # このファイル
├── research.md          # Phase 0 出力
├── data-model.md        # Phase 1 出力
├── quickstart.md        # Phase 1 出力
├── spec.md              # 機能仕様書
└── tasks.md             # Phase 2 出力 (/speckit.tasks)
```

### ソースコード (リポジトリルート)

```text
llmlb/tests/e2e-playwright/
├── helpers/
│   ├── api-helpers.ts            # [既存] APIヘルパー関数 → ユーザー管理・ログ・メトリクス用ヘルパー追加
│   ├── mock-helpers.ts           # [既存] モックデータヘルパー
│   ├── mock-openai-endpoint.ts   # [既存] → Audio/Image/Responses/xLLM/Ollama/vLLM モック追加
│   └── selectors.ts              # [既存] → 新UI要素のセレクタ追加
├── pages/
│   ├── auth.page.ts              # [既存] 変更なし
│   ├── dashboard.page.ts         # [既存] → Statistics/History/Logs/UserManagement メソッド追加
│   └── playground.page.ts        # [既存] → Playground設定・cURL メソッド追加
├── specs/
│   ├── auth/
│   │   ├── login.spec.ts             # [既存] 変更なし (7テスト)
│   │   ├── register.spec.ts          # [既存] 変更なし (8テスト)
│   │   ├── invitation.spec.ts        # [既存] 変更なし (6テスト)
│   │   └── session-management.spec.ts # [新規] FR-006: ログアウト・JWT期限切れ・セッション管理
│   ├── dashboard/
│   │   ├── dashboard-header.spec.ts      # [既存] 変更なし (7テスト)
│   │   ├── dashboard-nodes.spec.ts       # [既存] 変更なし (12テスト)
│   │   ├── dashboard-stats.spec.ts       # [既存] 変更なし (8テスト)
│   │   ├── endpoint-status-colors.spec.ts # [既存] 変更なし (1テスト)
│   │   ├── statistics-tab.spec.ts        # [新規] FR-001: トークン統計
│   │   ├── history-tab.spec.ts           # [新規] FR-002: リクエスト履歴
│   │   ├── logs-tab.spec.ts              # [新規] FR-003: ログビューア
│   │   ├── user-management.spec.ts       # [新規] FR-004: ユーザー管理
│   │   ├── endpoint-edit.spec.ts         # [新規] FR-005: エンドポイント編集
│   │   ├── endpoint-detail-viz.spec.ts   # [新規] FR-011: データビジュアライゼーション
│   │   ├── system-update-banner.spec.ts  # [新規] FR-010/FR-023: システム更新バナー
│   │   └── mobile-responsive.spec.ts     # [新規] FR-007: モバイルレスポンシブ
│   ├── api/
│   │   ├── error-handling.spec.ts           # [既存] 変更なし (8テスト)
│   │   ├── audio-api.spec.ts                # [新規] FR-012: Audio API
│   │   ├── image-api.spec.ts                # [新規] FR-013: Image API
│   │   ├── responses-api.spec.ts            # [新規] FR-014: Responses API
│   │   ├── sse-streaming.spec.ts            # [新規] FR-015: SSEストリーミング詳細
│   │   ├── permission-matrix.spec.ts        # [新規] FR-016: 権限マトリクス
│   │   ├── api-key-lifecycle.spec.ts        # [新規] FR-017: APIキーライフサイクル
│   │   └── prometheus-metrics.spec.ts       # [新規] FR-018: Prometheusメトリクス
│   └── workflows/
│       ├── api-key-openai-e2e.spec.ts          # [既存] 変更なし (1テスト)
│       ├── endpoint-playground-walkthrough.spec.ts # [既存] 変更なし (1テスト)
│       ├── lb-playground-walkthrough.spec.ts    # [既存] 変更なし (1テスト)
│       ├── model-registration.spec.ts           # [既存] 変更なし (5テスト)
│       ├── playground-settings.spec.ts          # [新規] FR-008/FR-009: Playground設定・cURL
│       ├── lb-load-balancing.spec.ts            # [新規] FR-019: LBロードバランシング
│       ├── endpoint-type-detection.spec.ts      # [新規] FR-020: タイプ検出
│       ├── model-download.spec.ts               # [新規] FR-021: モデルダウンロード
│       └── large-scale-load-test.spec.ts        # [新規] FR-022: 大規模負荷テスト
└── playwright.config.ts              # [既存] 変更なし
```

**構造決定**: 既存の`specs/{category}/`ディレクトリ構造を維持し、カテゴリ内に新規specファイルを追加。
ヘルパーとPage Objectは既存ファイルを拡張。モックサーバーは`mock-openai-endpoint.ts`に
マルチモーダルAPI対応を追加し、エンドポイントタイプ別応答パターンもオプションで追加。

## 複雑さトラッキング

> 憲章違反なし。新規ファイルは全てテストコードであり、既存パターンの拡張のみ。
