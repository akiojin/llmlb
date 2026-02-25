---
description: LLM主導のリリースフロー。バージョン更新→chore(release)コミット→タグ/Release→配信の一連の流れを実行します。
tags: [project]
---

# リリースコマンド（LLM主導・gwt スタイル）

LLM（Claude）がバージョン更新とリリースコミットを作成し、ワークフローがタグとリリースを作成します。

## フロー

```text
/release 実行
    ↓
① origin/develop を pull してローカルを最新化
    ↓
② LLMがバージョン更新（Cargo.toml, CHANGELOG.md）
    ↓
③ chore(release): vX.Y.Z コミット作成
    ↓
④ Closing Issue を収集（PR本文から Closes/Fixes/Resolves #N を抽出）
    ↓
⑤ develop → main マージ (PR本文に Closing Issues セクションを記載)
    ↓
⑥ release.yml がタグ作成 → GitHub Release作成
    ↓
⑦ publish.yml がバイナリビルド → Release にアタッチ
```

## 手順

### 0. ローカルを最新化（必須）

origin/develop の最新を取得し、ローカル develop を更新する：

```bash
git fetch origin
git pull origin develop
```

未コミットの変更がある場合はリリースを中断し、先に解決すること。

### 1. バージョン更新

Cargo.toml のバージョンを更新：

```toml
[workspace.package]
version = "X.Y.Z"  # 新しいバージョン
```

### 2. CHANGELOG.md 更新

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- 新機能の説明

### Fixed
- バグ修正の説明
```

### 3. リリースコミット作成

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore(release): vX.Y.Z"
git push origin develop
```

### 4. Closing Issue の収集

リリースPRに `Closes #N` を記載することで、main マージ時にIssueが自動クローズされる。

1. 前回リリースタグ〜HEADのコミットからPR番号を抽出：

   ```bash
   LAST_TAG=$(git describe --tags --abbrev=0)
   # スカッシュマージ: (#{number}) を抽出
   # マージコミット: Merge pull request #{number} を抽出
   PR_NUMBERS=$(git log ${LAST_TAG}..HEAD --oneline \
     | grep -oE '(#[0-9]+)|\bMerge pull request #[0-9]+' \
     | grep -oE '[0-9]+' | sort -u)
   ```

2. 各PRのボディから closing keyword を抽出：

   ```bash
   CLOSING_ISSUES=""
   for pr in $PR_NUMBERS; do
     BODY=$(gh pr view "$pr" --json body -q '.body' 2>/dev/null || true)
     ISSUES=$(echo "$BODY" \
       | grep -oiE '(close[sd]?|fix(e[sd])?|resolve[sd]?)\s+#[0-9]+' \
       | grep -oE '[0-9]+' || true)
     CLOSING_ISSUES="$CLOSING_ISSUES $ISSUES"
   done
   CLOSING_ISSUES=$(echo "$CLOSING_ISSUES" | tr ' ' '\n' | sort -u | grep -v '^$')
   ```

3. PR番号を除外し、真のIssueのみ残す：

   ```bash
   REAL_ISSUES=""
   for num in $CLOSING_ISSUES; do
     IS_PR=$(gh api "repos/{owner}/{repo}/issues/$num" \
       --jq 'has("pull_request") and .pull_request != null' 2>/dev/null || echo "false")
     if [ "$IS_PR" = "false" ]; then
       REAL_ISSUES="$REAL_ISSUES $num"
     fi
   done
   ```

4. 結果を確認し、次のステップのPR本文に使用する：

   ```bash
   for num in $REAL_ISSUES; do
     echo "Closes #$num"
   done
   ```

### 5. main へマージ

```bash
gh workflow run prepare-release.yml
# または手動でPR作成（Closing Issues セクションを含める）
gh pr create --base main --head develop \
  --title "chore(release): vX.Y.Z" \
  --body "$(cat <<'EOF'
## Summary

- リリース vX.Y.Z

## Release Notes

<!-- CHANGELOG.md の該当バージョンの内容を転記 -->

## Closing Issues

Closes #N
EOF
)"
```

### 6. 配布確認

- [GitHub Releases](https://github.com/akiojin/llmlb/releases) でリリースを確認
- バイナリが自動的にアタッチされることを確認

## 注意

- バージョンは [Semantic Versioning](https://semver.org/) に従う
- `chore(release):` プレフィックスが必須（release.yml のトリガー条件）
- GitHub CLI で認証済みであること（`gh auth login`）
