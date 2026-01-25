#!/usr/bin/env bash

# 機能完了: 自動マージワークフロー用のPull Request作成
#
# 使用法: ./finish-feature.sh [OPTIONS]
#
# オプション:
#   --draft         ドラフトPRとして作成（自動マージされない）
#   --help, -h      ヘルプメッセージを表示

set -e

DRAFT=false

for arg in "$@"; do
    case "$arg" in
        --draft)
            DRAFT=true
            ;;
        --help|-h)
            cat << 'EOF'
使用法: finish-feature.sh [OPTIONS]

Pull Requestを作成して機能開発を完了します。

オプション:
  --draft         ドラフトPRとして作成（自動マージされない）
  --help, -h      このヘルプメッセージを表示

ワークフロー:
  1. 現在のブランチがfeatureブランチ（'feature/'で始まる）であることを確認
  2. コミットされていない変更がないかチェック
  3. featureブランチをリモートにプッシュ
  4. GitHub Pull Requestを作成
  5. GitHub Actionsにより自動マージがトリガーされる

EOF
            exit 0
            ;;
        *)
            echo "エラー: 不明なオプション '$arg'。使用方法は --help を参照してください。" >&2
            exit 1
            ;;
    esac
done

# 共通関数を読み込み
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# リポジトリルートを取得
REPO_ROOT=$(get_repo_root)
cd "$REPO_ROOT"

# gitが利用可能かチェック
if ! has_git; then
    echo "エラー: Gitリポジトリが検出されません。このスクリプトにはgitが必要です。" >&2
    exit 1
fi

# 現在のブランチを取得
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)

# featureブランチにいることを確認
if [[ ! "$CURRENT_BRANCH" =~ ^feature/ ]]; then
    echo "エラー: featureブランチにいません。現在のブランチ: $CURRENT_BRANCH" >&2
    echo "featureブランチは 'feature/' で始まる必要があります" >&2
    exit 1
fi

# SPEC-IDを抽出（ブランチがSPEC命名規則に従っている場合）
SPEC_ID=""
if [[ "$CURRENT_BRANCH" =~ ^feature/SPEC-[a-z0-9]{8}$ ]]; then
    SPEC_ID=$(echo "$CURRENT_BRANCH" | sed 's/^feature\///')
fi

echo "========================================="
echo "機能完了: $CURRENT_BRANCH"
echo "========================================="

# コミットされていない変更をチェック
if ! git diff-index --quiet HEAD --; then
    echo ""
    echo "コミットされていない変更があります。先にコミットまたはスタッシュしてください。"
    echo ""
    git status --short
    exit 1
fi

# gh CLIがインストールされ認証されているかチェック
echo ""
echo "[1/4] GitHub CLIを確認中..."
if ! command -v gh &> /dev/null; then
    echo "エラー: GitHub CLI (gh) がインストールされていません。" >&2
    echo "インストール先: https://cli.github.com/" >&2
    exit 1
fi

if ! gh auth status &> /dev/null; then
    echo "エラー: GitHub CLIが認証されていません。" >&2
    echo "実行してください: gh auth login" >&2
    exit 1
fi

echo "✓ GitHub CLI準備完了"

# featureブランチをリモートにプッシュ
echo ""
echo "[2/4] featureブランチをリモートにプッシュ中..."
git push -u origin "$CURRENT_BRANCH"

# spec.mdからPRタイトルを取得
echo ""
echo "[3/4] Pull Requestを作成中..."
PR_TITLE="機能実装"

if [ -n "$SPEC_ID" ]; then
    SPEC_FILE="$REPO_ROOT/specs/$SPEC_ID/spec.md"
    if [ -f "$SPEC_FILE" ]; then
        # spec.mdからタイトルを抽出（マークダウンヘッダーを除去）
        PR_TITLE=$(head -1 "$SPEC_FILE" | sed 's/^# 機能仕様書: //' | sed 's/^# //')
    fi
else
    # 非SPECブランチの場合はブランチ名をタイトルとして使用
    PR_TITLE=$(echo "$CURRENT_BRANCH" | sed 's/^feature\///' | sed 's/-/ /g' | sed 's/\b\(.\)/\u\1/g')
fi

# PR本文を作成
if [ -n "$SPEC_ID" ]; then
    PR_BODY=$(cat <<EOF
## SPEC情報

**機能ID**: \`$SPEC_ID\`
**ブランチ**: \`$CURRENT_BRANCH\`

---

## 変更サマリー

$(git log origin/main..HEAD --oneline --no-merges | head -10)

---

## 自動品質チェック

このPRが作成されると、GitHub Actions **"Quality Checks"** ワークフローが自動実行されます：

### 並列実行されるチェック（5つ）

1. **tasks-check**: tasks.mdの全タスク完了チェック
2. **rust-test**: Rustテスト実行（ubuntu-latest, windows-latest）
3. **rust-lint**: Rust lintチェック（\`cargo fmt --check\`, \`cargo clippy\`）
4. **commitlint**: コミットメッセージ検証（Conventional Commits準拠）
5. **markdownlint**: マークダウンファイルlint

### 自動マージ条件

すべての品質チェックが合格すると、**"Auto Merge"** ワークフローが起動し、以下の条件を満たす場合に自動的にmainブランチへマージされます：

- ✅ 全品質チェックが成功
- ✅ PRがドラフトでない
- ✅ マージ可能（コンフリクトなし）
- ✅ マージ状態が正常（CLEAN または UNSTABLE）

---

## チェックリスト

- [ ] tasks.md の全タスクが完了している（\`- [x]\`）
- [ ] 全テストが合格している
- [ ] コンパイルエラーがない
- [ ] コミットメッセージがConventional Commits準拠

---

📝 **詳細**: \`specs/$SPEC_ID/spec.md\` を参照してください。

🤖 このPRは自動マージワークフローの対象です。品質チェック合格後、自動的にmainブランチへマージされます。
EOF
)
else
    PR_BODY=$(cat <<EOF
**ブランチ**: \`$CURRENT_BRANCH\`

---

## 変更サマリー

$(git log origin/main..HEAD --oneline --no-merges | head -10)

---

## 自動品質チェック

このPRが作成されると、GitHub Actions **"Quality Checks"** ワークフローが自動実行されます：

### 並列実行されるチェック（5つ）

1. **tasks-check**: tasks.mdの全タスク完了チェック
2. **rust-test**: Rustテスト実行（ubuntu-latest, windows-latest）
3. **rust-lint**: Rust lintチェック（\`cargo fmt --check\`, \`cargo clippy\`）
4. **commitlint**: コミットメッセージ検証（Conventional Commits準拠）
5. **markdownlint**: マークダウンファイルlint

### 自動マージ条件

すべての品質チェックが合格すると、**"Auto Merge"** ワークフローが起動し、以下の条件を満たす場合に自動的にmainブランチへマージされます：

- ✅ 全品質チェックが成功
- ✅ PRがドラフトでない
- ✅ マージ可能（コンフリクトなし）
- ✅ マージ状態が正常（CLEAN または UNSTABLE）

---

## チェックリスト

- [ ] 全テストが合格している
- [ ] コンパイルエラーがない
- [ ] コミットメッセージがConventional Commits準拠

---

🤖 このPRは自動マージワークフローの対象です。品質チェック合格後、自動的にmainブランチへマージされます。
EOF
)
fi

# PRを作成（ドラフトまたは通常）
if [ "$DRAFT" = true ]; then
    gh pr create --base main --head "$CURRENT_BRANCH" --title "$PR_TITLE" --body "$PR_BODY" --draft
    echo "✓ ドラフトPRを作成しました"
else
    gh pr create --base main --head "$CURRENT_BRANCH" --title "$PR_TITLE" --body "$PR_BODY"
    echo "✓ PRを作成しました"
fi

# PR URLを取得
PR_URL=$(gh pr view "$CURRENT_BRANCH" --json url --jq .url 2>/dev/null || echo "")

echo ""
echo "[4/4] クリーンアップ中..."
rm -f "$REPO_ROOT/.specify/.current-feature"

echo ""
echo "========================================="
if [ -n "$SPEC_ID" ]; then
    echo "✓ 機能 $SPEC_ID のPRを作成しました！"
else
    echo "✓ 機能PRを作成しました！"
fi
echo "========================================="
echo ""
if [ -n "$PR_URL" ]; then
    echo "PR URL: $PR_URL"
    echo ""
fi
echo "GitHub Actionsが品質チェックを実行します。"
echo "すべてのチェックが合格すると、PRは自動的にmainにマージされます。"
echo ""
if [ "$DRAFT" = true ]; then
    echo "注意: これはドラフトPRのため、自動マージされません。"
    echo "自動マージを有効にするには「Ready for review」に変更してください。"
fi
