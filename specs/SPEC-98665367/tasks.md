# タスク: CI/CD パイプライン

**機能ID**: `SPEC-98665367`
**ステータス**: 完了
**入力**: `specs/SPEC-98665367/spec.md`

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能（異なるファイル、依存関係なし）
- 説明には正確なファイルパスを含める

## Phase 1: 品質・テスト

- [x] T001 [P] `.github/workflows/ci.yml` に main 向け CI（tasks, fmt/clippy/test, markdownlint, coverage, mcp-server）を実装
- [x] T002 [P] `.github/workflows/lint.yml` に develop 向け lint（fmt/clippy/markdownlint/commitlint）を実装
- [x] T003 [P] `.github/workflows/test.yml` に develop 向けテスト（tasks, Rust tests, OpenAI proxy, Playwright）を実装

## Phase 2: PR/リリース制御

- [x] T004 [P] `.github/workflows/pr-gate.yml` に main 直PRの制御を実装
- [x] T005 [P] `.github/workflows/auto-merge.yml` に dependabot PR auto-merge を実装
- [x] T006 [P] `.github/workflows/prepare-release.yml` に develop -> main リリースPR作成/マージを実装

## Phase 3: リリース/配布

- [x] T007 [P] `.github/workflows/release.yml` にタグ作成 + GitHub Release 作成を実装
- [x] T008 [P] `.github/workflows/publish.yml` にマルチプラットフォーム配布と MCP server publish を実装

## Phase 4: Vision テスト

- [x] T009 [P] `.github/workflows/vision-tests.yml` に self-hosted GPU runner の Vision テストを実装

## Phase 5: 仕上げ

- [x] T010 `specs/SPEC-98665367/spec.md` のステータス整合（実装完了）
