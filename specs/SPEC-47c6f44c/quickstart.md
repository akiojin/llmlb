# クイックスタート: 自動マージ機能

**機能ID**: `SPEC-47c6f44c` | **日付**: 2025-10-30

## 概要

このガイドでは、自動マージ機能の動作を確認する手順を説明します。

## 前提条件

- [ ] GitHub CLIが認証済み (`gh auth login`)
- [ ] Rustプロジェクトのビルド環境が構成済み (`cargo`コマンドが利用可能)
- [ ] リポジトリの`main`ブランチにpush権限がある
- [ ] `.commitlintrc.json`が作成済み
- [ ] `.github/workflows/quality-checks.yml`が作成済み
- [ ] `.github/workflows/auto-merge.yml`が作成済み

## テストシナリオ1: 正常系（全チェック合格 → 自動マージ）

### ステップ1: テスト用featureブランチ作成

```bash
# リポジトリルートに移動
cd /ollama-coordinator

# テスト用featureブランチ作成
git checkout -b feature/test-auto-merge

# ダミー変更
echo "# Test auto-merge" >> TEST_AUTO_MERGE.md
git add TEST_AUTO_MERGE.md
git commit -m "feat: テスト用ファイル追加"
```

### ステップ2: tasks.mdに完了タスク追加（模擬）

```bash
# specs/SPEC-47c6f44c/tasks.mdに完了タスクを追加
cat >> specs/SPEC-47c6f44c/tasks.md <<EOF
## Test Tasks
- [x] Test task 1
- [x] Test task 2
EOF

git add specs/SPEC-47c6f44c/tasks.md
git commit -m "test: テストタスク追加"
```

### ステップ3: PR作成

```bash
# リモートにpush
git push -u origin feature/test-auto-merge

# PR作成
gh pr create --title "feat: 自動マージ機能テスト" --body "自動マージ機能の動作確認用PR

## チェックリスト
- [x] tasks.md完了
- [x] テスト実行
- [x] commitlint準拠"
```

### ステップ4: 品質チェック確認

```bash
# GitHub Actionsの実行確認
gh run list --branch feature/test-auto-merge

# 特定のワークフロー実行ログ確認
gh run view <RUN_ID>
```

**期待される結果**:

- Quality Checksワークフローが実行される
- tasks-check, rust-test, rust-lint, commitlint, markdownlintが並列実行される
- すべてのジョブが成功する

### ステップ5: 自動マージ確認

```bash
# Auto Mergeワークフローの実行確認
gh run list --workflow="Auto Merge"

# PRステータス確認
gh pr view feature/test-auto-merge
```

**期待される結果**:

- Auto Mergeワークフローが起動する
- PRが自動的にmainにマージされる
- featureブランチが削除される

## テストシナリオ2: 異常系（未完了タスク → 自動マージスキップ）

### ステップ1: テスト用featureブランチ作成

```bash
git checkout -b feature/test-tasks-fail

# ダミー変更
echo "# Test tasks fail" >> TEST_TASKS_FAIL.md
git add TEST_TASKS_FAIL.md
git commit -m "feat: 未完了タスクテスト"
```

### ステップ2: tasks.mdに未完了タスク追加

```bash
# 未完了タスクを追加
cat >> specs/SPEC-47c6f44c/tasks.md <<EOF
## Test Tasks (Incomplete)
- [ ] Incomplete task 1
- [ ] Incomplete task 2
EOF

git add specs/SPEC-47c6f44c/tasks.md
git commit -m "test: 未完了タスク追加"
```

### ステップ3: PR作成

```bash
git push -u origin feature/test-tasks-fail
gh pr create --title "test: 未完了タスクテスト" --body "未完了タスクがある場合の動作確認"
```

### ステップ4: 品質チェック確認

```bash
gh run list --branch feature/test-tasks-fail
```

**期待される結果**:

- Quality Checksワークフローが実行される
- tasks-checkジョブが**失敗**する
- 他のジョブは成功する
- 全体の結論は**failure**

### ステップ5: 自動マージスキップ確認

```bash
gh run list --workflow="Auto Merge"
gh pr view feature/test-tasks-fail
```

**期待される結果**:

- Auto Mergeワークフローは**起動しない**（条件不一致）
- PRはマージされない
- 開発者がtasks.mdを完了させる必要がある

## テストシナリオ3: ドラフトPR（品質チェック実行 → 自動マージスキップ）

### ステップ1: ドラフトPR作成

```bash
git checkout -b feature/test-draft
echo "# Test draft" >> TEST_DRAFT.md
git add TEST_DRAFT.md
git commit -m "feat: ドラフトPRテスト"

git push -u origin feature/test-draft

# ドラフトPR作成
gh pr create --title "feat: ドラフトPRテスト" --body "ドラフトPRの動作確認" --draft
```

### ステップ2: 品質チェック確認

```bash
gh run list --branch feature/test-draft
```

**期待される結果**:

- Quality Checksワークフローが実行される
- すべてのジョブが成功する

### ステップ3: 自動マージスキップ確認

```bash
gh run list --workflow="Auto Merge"
```

**期待される結果**:

- Auto Mergeワークフローは起動する
- しかし、`isDraft == true`のため**マージスキップ**される
- ログに「PR is a draft」と記録される

### ステップ4: ドラフト解除 → 自動マージ

```bash
# ドラフト解除
gh pr ready feature/test-draft

# 品質チェック再実行（自動）
gh run list --branch feature/test-draft

# 自動マージ確認
gh pr view feature/test-draft
```

**期待される結果**:

- 品質チェック再実行後、Auto Mergeが起動
- 今度は`isDraft == false`なのでマージ実行される

## トラブルシューティング

### 問題1: Auto Mergeワークフローが起動しない

**原因**: `workflow_run`トリガーはデフォルトブランチ（main）のワークフローファイルを使用

**解決策**:

1. `.github/workflows/auto-merge.yml`がmainブランチに存在することを確認
2. quality-checks.ymlのワークフロー名が正確に「Quality Checks」であることを確認

### 問題2: commitlintが失敗する

**原因**: コミットメッセージが規約違反

**解決策**:

```bash
# コミットメッセージを修正
git commit --amend -m "feat: 正しい形式のコミットメッセージ"
git push --force-with-lease
```

### 問題3: tasks-checkが失敗する

**原因**: tasks.mdに未完了タスク（`- [ ]`）が存在

**解決策**:

```bash
# tasks.mdを編集して全タスクを完了にする
vim specs/SPEC-47c6f44c/tasks.md
# - [ ] → - [x] に変更

git add specs/SPEC-47c6f44c/tasks.md
git commit -m "chore: タスク完了"
git push
```

## 検証チェックリスト

- [ ] 正常系: 全チェック合格 → 自動マージ成功
- [ ] 異常系: 未完了タスク → tasks-check失敗
- [ ] 異常系: 規約違反コミット → commitlint失敗
- [ ] ドラフトPR: 品質チェック実行 → 自動マージスキップ
- [ ] ドラフト解除: 自動マージ実行
- [ ] コンフリクト: 自動マージスキップ
- [ ] マージ後: featureブランチ削除確認

## 次のステップ

検証が完了したら、以下を実行：

1. テスト用PRをすべてクローズ
2. テスト用ブランチを削除
3. `CLAUDE.md`の自動マージセクションを更新
4. `finish-feature.sh`のPRボディを更新

---

**クイックスタート完了日**: 2025-10-30
