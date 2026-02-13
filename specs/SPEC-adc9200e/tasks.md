# SPEC-adc9200e: タスク一覧

## Setup

- [x] S-1: PLANS.md作成・更新

## Test (RED)

- [x] T-1: ghostバリアント廃止後のビルドエラーテスト準備
  - button.tsxからghost定義を削除した際にTypeScriptコンパイルエラーで
    全使用箇所が検出されることを確認する手順を準備

## Core

- [x] C-1: button.tsx 共通ベーススタイル改修 [P]
  - transition-all → 明示的プロパティ指定
  - duration-200 → duration-150（hover）/ duration-75（active）
  - disabled状態: opacity-40 + cursor-not-allowed + saturate-0
  - focus-visible: ring-foreground + ring-offset-4 + ring-offset-background

- [x] C-2: button.tsx defaultバリアント改修 [P]
  - hover: bg-primary/85 + shadow-lg + -translate-y-0.5
  - active: scale-[0.95] + shadow-none + translate-y-0

- [x] C-3: button.tsx destructiveバリアント改修 [P]
  - hover: bg-destructive/85 + shadow-lg + -translate-y-0.5
  - active: scale-[0.95] + shadow-none + translate-y-0

- [x] C-4: button.tsx outlineバリアント改修 [P]
  - hover: shadow-md + -translate-y-0.5
  - active: scale-[0.95] + shadow-none + translate-y-0 + bg-accent/80

- [x] C-5: button.tsx secondaryバリアント改修 [P]
  - hover: bg-secondary/70 + shadow-md + -translate-y-0.5
  - active: scale-[0.95] + shadow-none + translate-y-0

- [x] C-6: button.tsx linkバリアント改修 [P]
  - hover: text-primary/80
  - active: text-primary/60

- [x] C-7: button.tsx glowバリアント改修 [P]
  - hover: -translate-y-0.5
  - active: scale-[0.95] + shadow-none + translate-y-0

- [x] C-8: button.tsx ghostバリアント削除
  - 依存: C-1〜C-7完了後
  - variants定義からghost削除
  - TypeScript型からghostオプション除去

## Integration

- [x] I-1: Header.tsx ghost→outline置換（3箇所）
  - 依存: C-8
  - Refresh Button, Theme Toggle, User Menu Trigger

- [x] I-2: EndpointTable.tsx ghost→outline置換（4箇所） [P]
  - 依存: C-8
  - Details, Test, Sync, Delete アイコンボタン

- [x] I-3: RequestHistoryTable.tsx ghost→outline置換（2箇所） [P]
  - 依存: C-8

- [x] I-4: ApiKeyModal.tsx ghost→outline置換（3箇所） [P]
  - 依存: C-8

- [x] I-5: InvitationModal.tsx ghost→outline置換（1箇所） [P]
  - 依存: C-8

- [x] I-6: UserModal.tsx ghost→outline置換（2箇所） [P]
  - 依存: C-8

- [x] I-7: EndpointPlayground.tsx ghost→outline置換（2箇所） [P]
  - 依存: C-8

- [x] I-8: LoadBalancerPlayground.tsx ghost→outline/default置換（4箇所）
  - 依存: C-8
  - モード切替ボタン: 非アクティブ時ghost→outline
  - Settings: ghost→outline
  - Copy cURL: ghost→outline

- [x] I-9: Dashboard.tsx 生button→Buttonコンポーネント化（2箇所）
  - "Restart to update": `<button>` → `<Button>`
  - "Try again": `<button>` → `<Button variant="link">`

- [x] I-10: EndpointPlayground.tsx 生button→Buttonコンポーネント化（2箇所） [P]
  - 添付ファイル削除ボタン（画像・音声）

- [x] I-11: LoadBalancerPlayground.tsx 生button→Buttonコンポーネント化（1箇所） [P]
  - 添付ファイル削除ボタン

## Polish

- [x] P-1: ダッシュボードビルド成功確認
  - `pnpm --filter @llm/dashboard build`
  - ビルド成果物を `llmlb/src/web/static/` にコミット

- [x] P-2: ghost参照残存チェック
  - `grep -r "ghost" llmlb/src/web/dashboard/src/` でghost参照がないことを確認

- [x] P-3: cargo test 実行・全テスト通過確認

- [x] P-4: make quality-checks 全体検証
