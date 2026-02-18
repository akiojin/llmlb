# 実装計画: CI/CD パイプライン

**機能ID**: `SPEC-5da87697` | **日付**: 2026-02-03 | **仕様**: `specs/SPEC-5da87697/spec.md`
**入力**: `specs/SPEC-5da87697/spec.md`, `.github/workflows/*.yml`

## 実行フロー (/speckit.plan スコープ)
- 既存ワークフローの一覧化
- 目的/トリガー/依存関係の整理
- 受け入れ条件と運用ルールの明文化

## 概要
GitHub Actions を用いて、品質チェック、PR制御、リリース作成、バイナリ配布、
MCPサーバー公開を統合する。main/develop の分岐運用と
releaseブランチ運用を前提に、各ワークフローの責務を分離する。

## 既存ワークフロー構成

### 品質・テスト
- `ci.yml` (main): tasksチェック、Rust fmt/clippy/test、markdownlint、coverage、mcp-server
- `lint.yml` (develop): Rust fmt/clippy、markdownlint、commitlint
- `test.yml` (develop): tasksチェック、Rust tests (Ubuntu/Windows)、OpenAI互換APIテスト、Playwright E2E

### PR/リリース制御
- `pr-gate.yml`: main への PR は release/* ブランチのみ許可
- `prepare-release.yml`: develop -> main の PR 作成/自動マージ
- `auto-merge.yml`: dependabot PR の auto-merge

### リリース/配布
- `release.yml`: main へのリリースコミット/手動でタグ作成 & GitHub Release 作成
- `publish.yml`: タグ起点でマルチプラットフォームバイナリをビルド/配布
  - Linux (musl), Windows (msvc), macOS (intel/arm)
  - MCPサーバー npm publish

## 依存関係と権限
- Secrets: `PERSONAL_ACCESS_TOKEN`, `GITHUB_TOKEN`, `NPM_TOKEN`
- submodules を含む checkout を前提

## 受け入れ条件
- PR作成時に品質チェックが自動実行される
- develop -> main のリリースフローが自動化されている
- main リリースでタグ/リリース/配布が連動する
- dependabot PR の auto-merge が機能する

## リスク/運用
- release タグ作成の二重実行を防止するガードが必要
- develop/main の運用ルール逸脱を PR gate で抑止する
