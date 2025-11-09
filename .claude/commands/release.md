---
description: developからrelease/vX.Y.Zブランチを作成し、リリースフローを開始します。
tags: [project]
---

# リリースコマンド

developブランチから`release/vX.Y.Z`ブランチを自動作成し、正式リリースフローを開始します。

## 実行内容

1. 現在のブランチがdevelopであることを確認
2. developブランチを最新に更新（`git pull`）
3. semantic-releaseのドライランで次バージョンを判定
4. `release/vX.Y.Z`ブランチをdevelopから作成
5. リモートにpush
6. GitHub Actionsが以下を自動実行：
   - **releaseブランチ**: semantic-releaseによりCHANGELOG/Cargo.toml/タグ/GitHub Releaseを更新し、releaseブランチをmainへ直接取り込み（バックマージでdevelopも同期）
   - **mainブランチ**: release.ymlの完了後にpublish.ymlが起動し、`release-binaries.yml`を呼び出して各プラットフォーム向けバイナリを添付

## 前提条件

- developブランチにいること
- GitHub CLIが認証済みであること（`gh auth status`）
- コミットがConventional Commits準拠で、semantic-releaseがバージョンを判定できること

## スクリプト実行

以下のスクリプトを実行してリリースブランチを作成します：

```bash
scripts/create-release-branch.sh
```

スクリプトはGitHub Actionsの`create-release.yml`を起動し、リモートで次を実行します：

1. developでsemantic-releaseドライラン
2. 次バージョン番号の決定
3. `release/vX.Y.Z`ブランチの作成とpush

その後 release.yml → publish.yml → release-binaries.yml が連鎖的に進み、各プラットフォーム向け成果物を含むリリースが完了します。
