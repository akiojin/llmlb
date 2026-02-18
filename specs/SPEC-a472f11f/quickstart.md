# Phase 1 クイックスタート: 包括的E2Eテストスイート強化

**機能ID**: `SPEC-a472f11f` | **日付**: 2026-02-13

## 前提条件

- llmlbがビルド済み (`cargo build -p llmlb`)
- Node.js + pnpm がインストール済み
- Playwright ブラウザがインストール済み (`pnpm --filter @llm/e2e-playwright exec playwright install chromium`)

## テスト実行手順

### 全テスト実行 (headed)

```bash
cd llmlb/tests/e2e-playwright
SKIP_SERVER=1 npx playwright test --headed
```

### カテゴリ別実行

```bash
# 認証テスト
npx playwright test specs/auth/ --headed

# ダッシュボードUIテスト
npx playwright test specs/dashboard/ --headed

# APIテスト
npx playwright test specs/api/ --headed

# ワークフローテスト
npx playwright test specs/workflows/ --headed
```

### 新規テスト個別実行

```bash
# 権限マトリクス
npx playwright test specs/api/permission-matrix.spec.ts --headed

# SSEストリーミング
npx playwright test specs/api/sse-streaming.spec.ts --headed

# LBロードバランシング
npx playwright test specs/workflows/lb-load-balancing.spec.ts --headed
```

## 実装順序

### Step 1: モックサーバー拡張

`helpers/mock-openai-endpoint.ts` にAudio/Image/Responses/タイプ別応答を追加。
全ての新規テストの基盤となるため、最優先。

### Step 2: ヘルパー関数追加

`helpers/api-helpers.ts` にユーザー管理・APIキー管理・ログ取得・メトリクス取得ヘルパーを追加。

### Step 3: Page Object拡張

`pages/dashboard.page.ts` にStatistics/History/Logs/UserManagement/EndpointEdit操作メソッドを追加。
`pages/playground.page.ts` にPlayground設定・cURL操作メソッドを追加。

### Step 4: テストスペック作成

TDDサイクルに従い、テスト作成(RED) → 必要に応じて実装確認(GREEN) → リファクタリング(REFACTOR)。
テストはUI機能→API→インテグレーションの順で作成。

## ディレクトリ構造

```text
llmlb/tests/e2e-playwright/
├── helpers/        # 共通ヘルパー（モック・API・セレクタ）
├── pages/          # Page Objectモデル
├── specs/
│   ├── auth/       # 認証テスト
│   ├── dashboard/  # ダッシュボードUIテスト
│   ├── api/        # APIテスト
│   └── workflows/  # ワークフロー・インテグレーションテスト
└── playwright.config.ts
```
