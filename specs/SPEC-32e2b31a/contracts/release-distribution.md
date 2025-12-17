# Release Distribution Contract

## 目的

GitHub Release に添付される配布物が、プラットフォームごとに正しいアーカイブ形式と同梱ファイルセットを満たしていることを保証する。

## 成果物

- `.github/workflows/release.yml` / `.github/workflows/publish.yml` における自動検証ステップ
- リリースノート作成時に参照するチェックリスト

## 検証項目

| ID | 観点 | 詳細 | テスト方法 |
| --- | --- | --- | --- |
| RD-01 | アーカイブ形式 | Linux/macOS 向けは `.tar.gz`、Windows 向けは `.zip` で生成される | ワークフロー内で拡張子パターンを `test` コマンドで検証 |
| RD-02 | 同梱ファイル | Router アーカイブには `llm-router`（Windowsは `.exe`）と `README.md` / `README.ja.md` / `LICENSE` が含まれる | `tar -tzf` / `unzip -Z1` でファイル一覧を確認 |
| RD-03 | 命名規則 | アーカイブ名は `llm-router-<platform>` / `llm-node-<platform>` で始まる | ワークフロー内で `[[ $archive == llm-router-* ]]` 等を確認 |
| RD-04 | リリースタイミング | リリースビルドは `main` ブランチにマージ済みのコミットのみを対象とする | ワークフロー先頭で `target_commitish == "main"` を検証 |
| RD-05 | インストーラー構成 | macOS `.pkg` / Windows `.msi` は Router/Node インストーラーを生成する（`llm-router-*` / `llm-node-*`） | `.github/workflows/publish.yml` の installer ステップが Router/Node 成果物をビルド・アップロードするか確認 |

## チェックリスト

1. 新しいターゲットを追加する場合、RD-01〜RD-03 を更新したか
2. `.github/workflows/release.yml` / `.github/workflows/publish.yml` の検証ステップで新ターゲット分の条件を追加したか
3. 手動でリリースする際も、同梱ファイルとフォーマットを必ず確認したか
4. リリース対象コミットが `main` ブランチに存在することを確認したか
5. Router/Node インストーラーを出力する設定（RD-05）を更新したか
