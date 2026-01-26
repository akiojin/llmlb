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
① LLMがバージョン更新（Cargo.toml, CHANGELOG.md）
    ↓
② chore(release): vX.Y.Z コミット作成
    ↓
③ develop → main マージ (prepare-release.yml または手動PR)
    ↓
④ release.yml がタグ作成 → GitHub Release作成
    ↓
⑤ publish.yml がバイナリビルド → Release にアタッチ
```

## 手順

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

### 4. main へマージ

```bash
gh workflow run prepare-release.yml
# または手動でPR作成
gh pr create --base main --head develop --title "chore(release): vX.Y.Z"
```

### 5. 配布確認

- [GitHub Releases](https://github.com/akiojin/llmlb/releases) でリリースを確認
- バイナリが自動的にアタッチされることを確認

## 注意

- バージョンは [Semantic Versioning](https://semver.org/) に従う
- `chore(release):` プレフィックスが必須（release.yml のトリガー条件）
- GitHub CLI で認証済みであること（`gh auth login`）
