正式リリースプロセスを開始します。

## 概要

このコマンドは `scripts/release/create-release-branch.sh` を実行し、
`akiojin/unity-mcp-server` と同じ方式の自動リリース（release/vX.Y.Zブランチ → release.yml → publish.yml）を起動します。

## 実行内容

1. **GitHub CLIの状態確認**
   - `gh` コマンドの存在と `gh auth status` を確認
2. **create-release.yml の実行**
   - developブランチを元に semantic-release のドライランで次バージョンを決定
   - `release/vX.Y.Z` ブランチを自動作成＆push
3. **自動処理**（ブランチ作成後）
   - releaseブランチのpush → `release.yml`
     - semantic-release本番実行
     - バージョンタグ + CHANGELOG + Cargo.toml更新
     - mainへの自動マージ／developへのバックマージ／releaseブランチ削除
   - mainへのpush → `publish.yml`
     - `release-binaries.yml` を呼び出し、全プラットフォームのバイナリをGitHub Releaseへ添付

## 使用方法

以下を実行してください：

```bash
./scripts/release/create-release-branch.sh
```

Claude Codeからは：

```
/release
```

## 注意事項

- GitHub CLIが認証済みであること (`gh auth status`)
- releaseブランチは `release/vX.Y.Z` 形式で単一実行（重複作成禁止）
- developブランチにリリース対象の変更が揃っていること

## トラブルシューティング

### バージョンが検出できない
- developにConventional Commits準拠の変更があるか確認
- 直前のリリースからの差分がない場合はreleaseブランチを作成できません

### ワークフローの進捗確認

```bash
gh run watch \$(gh run list --workflow=create-release.yml --limit 1 --json databaseId --jq '.[0].databaseId')
```

release.yml / publish.yml の進行状況も同様に `gh run watch` で確認できます。

---

実行しますか？ (このプロンプトを確認後、スクリプトを実行します)
