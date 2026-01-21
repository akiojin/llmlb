# CLAUDE.md

このファイルは、このリポジトリでコードを扱う際のガイダンスを提供します。
短く「何を・どこを見るか」を示し、詳細は既存ドキュメントに段階的に委譲します
（参考: [Writing a good CLAUDE.md](https://www.humanlayer.dev/blog/writing-a-good-claude-md)）。

## まず読む 90秒版

- 何を作る: Rust製ルーター（`router/`）＋ llama.cppベースのC++推論エンジン（`allm/`）。Ollamaは一切使わない／復活させない。
- どこを見る: `README.md`（全体像）→ `DEVELOPMENT.md`（セットアップ）→ `specs/`（要件とタスク）。
- 守る: ブランチ／worktree作成・切替禁止、作業ディレクトリ移動禁止、GPU非搭載エンドポイント登録禁止、必ずローカルで全テスト実行。
- HFカタログ利用時は`HF_TOKEN`（任意）と必要に応じ`HF_BASE_URL`を環境にセットしておく。
- まず実行: `make quality-checks`（時間がない場合でも個別コマンドを全て回すこと）。
- 迷ったら: `memory/constitution.md`とこのファイル後半の詳細ルールを再確認。
- 回答は日本語で行う。

## ディレクトリ構成

```text
llm-router/
├── router/          # Rust製ルーター（APIサーバー・管理UI）
├── allm/            # C++製推論エンジン（llama.cppベース）
├── common/          # 共通ライブラリ（Rust/C++間共有）
├── specs/           # 機能仕様書（SPEC-XXXXXXXX/）
├── memory/          # プロジェクト憲章・メモリファイル
├── docs/            # ドキュメント
├── mcp-server/      # MCPサーバー（Claude Desktop連携）
├── poc/             # 概念実証・実験コード
├── vendor/          # サードパーティ依存（サブモジュール）
└── .specify/        # Spec Kit設定・スクリプト
```

## 用語定義

| 用語 | 説明 |
|------|------|
| **ルーター** | Rust製のAPIゲートウェイ。OpenAI互換APIを提供し、リクエストを適切なエンドポイントに振り分ける。ダッシュボード（管理UI）も内蔵。 |
| **エンドポイント** | ルーターが管理する推論サービスの接続先。aLLM、Ollama、vLLM、その他OpenAI互換APIなど多様なバックエンドを統一的に扱う。 |
| **aLLM** | 本プロジェクト独自のC++製推論エンジン。llama.cpp/whisper.cpp/stable-diffusion.cppなどを統合し、GPUを活用したローカル推論を提供。vLLM/Ollamaと同列のエンドポイントタイプとして扱う。 |

## システムアーキテクチャ

### ルーター（`router/` - Rust製）

リクエストの受付・振り分け・管理を担当するコンポーネント。

- **OpenAI互換APIの提供**: `/v1/chat/completions`, `/v1/embeddings`等のエンドポイント
- **認証**: JWT認証（ダッシュボード）、APIキー認証（API）
- **エンドポイント管理**: 複数エンドポイントの登録・ヘルスチェック・負荷分散
- **モデルカタログ**: HuggingFaceカタログとの連携、モデルメタデータ管理
- **リクエストルーティング**: 適切なエンドポイントへのリクエスト転送
- **ダッシュボード**: 管理UI（SPA）の提供

### aLLM（`allm/` - C++製、llama.cppベース）
- NOTE: llama.cpp is pinned to the akiojin/llama.cpp fork until upstream fixes land; once upstream is fixed, switch back to ggerganov/llama.cpp.

実際のLLM推論を担当するコンポーネント。vLLM/Ollamaと同列のエンドポイントタイプ。

- **モデルロード**: GGUFファイルのロード・VRAM管理
- **推論実行**: テキスト生成、埋め込み計算
- **GPU管理**: VRAM監視、モデルのロード/アンロード
- **ストリーミング**: トークン単位のレスポンス
- **エンジン管理**: Text/Audio/Image のマネージャで推論エンジンを静的に管理（SPEC-d7feaa2c）

### エンジン選択方針（プロジェクトルール）

| モデル正本 | safetensors.cpp | llama.cpp（GGUF） |
|-----------|-----------------|-------------------|
| safetensors（OpenAI, NVIDIA公式） | 必須（テスト必須） | 追加サポート（サードパーティGGUF版） |
| GGUF（Meta公式等） | 不要 | 必須 |

- **正本がsafetensorsのモデル**（gpt-oss, Nemotron 3等）:
  - safetensors.cppで動作必須（Metal/CUDA対応必須）
  - GGUF版も動作可能（llama.cpp、サードパーティ変換版）
- **正本がGGUFのモデル**（Llama, Mistral等）:
  - llama.cppで対応（Metal/CUDA対応済み）
  - safetensors.cppでの対応は不要

### 動作モード

**aLLM単体モード（スタンドアロン）**:

```text
[クライアント] → [aLLM (GPU)]
```

- Ollamaサーバーと同様のスタンドアロン動作
- 単一マシン・単一GPUでのシンプルな運用
- ローカル開発や小規模利用に適している

**ルーター + エンドポイント モード（分散構成）**:

```text
[クライアント] → [ルーター] → [aLLM1 (GPU)]
                          → [aLLM2 (GPU)]
                          → [Ollama]
                          → [vLLM]
```

- 複数エンドポイントの統合管理
- 負荷分散・スケールアウト
- 異種バックエンド（aLLM/Ollama/vLLM等）の混在運用
- 大規模運用・本番環境向け

## 絶対原則（Kaguyaワークフロー準拠）

> **要件・仕様がない状態での実装は絶対にNG**
>
> 仕様書（`specs/SPEC-XXXXXXXX/`）が完成するまで、TDD RED（テスト作成）に進んではならない。

## 参照リンク（詳細はここで確認）

- プロジェクト概要とセットアップ: `README.md`, `README.ja.md`, `DEVELOPMENT.md`
- 仕様とタスク: `specs/` 配下の `spec.md` / `plan.md` / `tasks.md`
- 品質基準と憲章: `memory/constitution.md`
- CLI とモデル管理: `router/src/cli/`
- テスト＆CIワークフロー: `.specify/scripts/checks/`, `make quality-checks`, `make openai-tests`

## よくあるNG（必ず回避）
- サブモジュールの修正は基本的にNG（必要時は事前に明示的な承認を取る）

- Ollama を再導入する変更
- ブランチ／worktree操作・`cd` での作業ディレクトリ移動
- テストをスキップしたコミット／プッシュ
- Spec を書かずに新機能を実装すること
- OSS をベンダー取り込みして編集すること（原則サブモジュール運用）
- 廃止機能を「後方互換」名目でコードやテストに残すこと（廃止が決まったら完全削除する）
- ダミー/フェイク/モック実装を本番コードに含めること（テスト専用コードは例外）
- 検証やチェックをスキップするための環境変数やフラグを使用すること
- Task toolやTaskOutputで`cargo fmt --check`、`make quality-checks`等の長時間実行コマンドを実行すること（コンテキストを大量消費しLLMがクラッシュするため、直接Bashツールで出力制限付きで実行すること。詳細は「ローカル検証」セクション参照）

## 現在の目的

- `specs/` 配下で定義された要件・タスクの未完了項目を洗い出し、順次完了させることを最優先とする
- 作業中は最新のSpecと整合するようにテスト・ドキュメント・実装を更新し、完了後は必ずチェックリストを更新する

## 開発指針

### 🛠️ 技術実装指針

- **設計・実装は複雑にせずに、シンプルさの極限を追求してください**
- **ただし、ユーザビリティと開発者体験の品質は決して妥協しない**
- 実装はシンプルに、開発者体験は最高品質に
- CLI操作の直感性と効率性を技術的複雑さより優先
- GPU未搭載エンドポイントは登録させない。API・UI・テストでGPUデバイス情報を必須とする。
- 要件を満たすOSSライブラリが存在する場合は、車輪の再発明を避け、優先的に採用する
- OSSはサブモジュールとして管理し、原則変更しない（変更が必要ならフォーク運用）

### 📝 設計ガイドライン

- 設計に関するドキュメントには、ソースコードを書かないこと

### 🔐 開発モードの認証方針

開発モード（デバッグビルド）でも認証フローをスキップせず、正規の認証処理を通す。

- **JWT認証（ダッシュボード）**: `admin` / `test` でログイン
- **APIキー認証（OpenAI互換API）**: `sk_debug` を使用

これらのデバッグ用認証情報は `#[cfg(debug_assertions)]` で保護され、リリースビルドでは無効化される。認証チェックをスキップする環境変数やフラグは禁止。

## 開発品質

### 完了条件

- エラーが発生している状態で完了としないこと。必ずエラーが解消された時点で完了とする。

## 開発ワークフロー

### Kaguya準拠の作業手順（本プロジェクト向けに調整）

#### Step 0: 作業確認（必須）
1. `PLANS.md` を確認し、このブランチで対応すべき内容を把握する（差分があれば更新）
2. `specs/SPEC-XXXXXXXX/` を確認し、仕様書・計画・タスクの現状を確認する

#### Step 1: 仕様策定（TDD RED の前に必須）

| 順序 | コマンド | 生成物 |
|------|----------|--------|
| 1 | `/speckit.specify <機能説明>` | `spec.md` |
| 2 | `/speckit.clarify SPEC-XXX` | 不明点解消 |
| 3 | `/speckit.plan SPEC-XXX` | `plan.md`, `data-model.md`, `research.md`, `quickstart.md` |
| 4 | `/speckit.tasks SPEC-XXX` | `tasks.md` |

**ここまで完了して初めて Step 2 に進める。**

#### Step 2: 実装（TDD）
1. Worktree/ブランチの作成・切替は行わない（この環境は既に準備済み）
2. `PLANS.md` を作成/更新し、目的・対応SPECを明記する
3. TDD サイクル:
   - **RED**: テスト作成・失敗確認
   - **GREEN**: 最小実装
   - **REFACTOR**: 品質向上
4. `PLANS.md` を進捗に応じて更新する

#### Step 3: 完了
1. ユーザー承認
2. マージはリポジトリメンテナが実施（必要に応じて指示を仰ぐ）

### 基本ルール

- 作業（タスク）を完了したら、変更点を日本語でコミットログに追加して、コミット＆プッシュを必ず行う
- コミットメッセージは commitlint (Conventional Commits) に準拠した形式で記述する（例: `feat(core): add new api`）
- featureブランチからのPRは必ず`develop`ブランチをベースに作成する（`main`への直接PRは禁止）
- **作業開始前に `PLANS.md` に今後の対応を書き出し、作業中に変更があれば随時更新する**
- **`PLANS.md` はブランチ単位のローカル作業メモとして運用し、Git管理対象外（.gitignore）とする**
- 作業（タスク）は、最大限の並列化をして進める
- 作業（タスク）は、最大限の細分化をしてToDoに登録する
- 作業（タスク）の開始前には、必ずToDoを登録した後に作業を開始する
- 作業（タスク）は、忖度なしで進める
- 作業開始前に `PLANS.md` を確認し、差分があれば更新してから着手する
- `PLANS.md` はGit管理外（ローカル運用）のため、コミット対象に含めない

### PLANS.md 運用手順（作業開始前の必須手順）

1. 作業を始める前に `PLANS.md` を開き、当日の日付に「今後の対応」を箇条書きで追記する
   - 未記入の場合は作業を開始しない
2. 着手前にToDoへ分解し、`PLANS.md` の項目と対応関係が分かるように保つ
3. 作業中に優先度や方針が変わったら、必ず `PLANS.md` を更新する
4. タスク完了後は、完了済みの更新内容が `PLANS.md` に反映されていることを確認する
5. `PLANS.md` はブランチ単位のローカルメモとして扱い、コミットしない（.gitignoreで管理外）

### 作業開始前の確認（PLANS徹底）

- `PLANS.md` の「今後の対応」を更新済みでなければ**作業に着手しない**
- 更新内容は本日の作業と一致していることを確認する
- 記載漏れがあれば**先にPLANSを更新**してから作業を進める
- `PLANS.md` はローカル専用で、Git管理対象外であることを確認する

### 環境固定ルール（プロジェクトカスタム）

- 勝手にブランチ作成やWorktree作成は禁止（`git branch`, `git worktree add` などを実行しない）
- 作業ディレクトリの変更は禁止（`cd`による移動を行わない）
- 現在のブランチの切り替えは禁止（`git switch`, `git checkout` などを実行しない）
- ブランチやWorktreeの作成・切り替えはリポジトリメンテナが必要時に実施する。作業者は現状の環境を維持してタスクを完了させること。

### ローカル検証（絶対厳守）

GitHub Actions が実行する検証を**全てローカルで事前に成功させてから**コミットすること。例外は認めない。

- 下記コマンド群を現在の作業環境で順番に実行し、すべて成功（終了コード0）を確認すること
  - `cargo fmt --check`
  - `cargo clippy -- -D warnings`
  - `cargo test`
  - `.specify/scripts/checks/check-tasks.sh`
  - `pnpm dlx markdownlint-cli2 "**/*.md" "!node_modules" "!.git" "!.github" "!.worktrees"`
- コミット対象に応じて `.specify/scripts/checks/check-commits.sh` やその他ワークフロー相当のスクリプト
- まとめて実行する場合は `make quality-checks`（OpenAI互換APIテスト `make openai-tests` を内包）を推奨。
- OpenAI互換APIのみを個別に確認したい場合は `make openai-tests` を実行すること。
- いずれかが失敗した状態でコミットすることを固く禁止する。失敗原因を解消し、再実行→合格を確認してからコミットせよ。
- ローカル検証結果を残すため、必要に応じて実行ログをメモし、レビュー時に提示できるようにすること。

#### ⚠️ コンテキスト消費を抑える実行方法（Claude Code向け）

Task toolやバックグラウンドタスクで品質チェックを実行すると、大量の出力がコンテキストに蓄積されLLMがクラッシュする可能性がある。**必ず直接Bashツールで、出力を制限して実行すること。**

```bash
# ❌ NG: Task toolやバックグラウンドで実行
# ❌ NG: 出力制限なしで実行

# ✅ OK: 直接Bashで出力制限付き実行（推奨パターン）

# フォーマットチェック（成功/失敗のみ確認）
cargo fmt --check > /dev/null 2>&1 && echo "✓ fmt OK" || echo "✗ fmt FAIL"

# Clippy（最後の20行のみ表示）
cargo clippy -- -D warnings 2>&1 | tail -20

# テスト（サマリのみ表示）
cargo test 2>&1 | grep -E "(test result|FAILED|passed|failed)" | tail -10

# markdownlint（エラー数のみ確認）
pnpm dlx markdownlint-cli2 "**/*.md" "!node_modules" "!.git" "!.github" "!.worktrees" 2>&1 | tail -10

# 全体を一括確認（出力制限付き）
make quality-checks 2>&1 | tail -50
```

#### ⚠️ テスト実行時間の目安（重要）

以下のコマンドは実行に時間がかかるため、タイムアウトを適切に設定すること：

| コマンド | 所要時間 | 推奨timeout |
|----------|----------|-------------|
| `cargo test -- --test-threads=1` | **約8-10分**（265テスト） | 600000ms |
| `make quality-checks` | **約10-15分** | 900000ms |

**絶対禁止事項：**

- ❌ タイムアウトを「ハング」と誤解して `git commit --no-verify` でバイパスする
- ❌ pre-commitフックの問題を調査せずにスキップする

**正しい対応：**

- ✅ タイムアウトした場合は、より長いtimeoutを設定して再実行
- ✅ 本当にハングしている場合は、どのテストで止まっているか特定してから対処

### commitlint準拠コミットログ（絶対厳守・バージョニング直結）

**🚨 重要: このプロジェクトはsemantic-releaseによる自動バージョン管理を採用しています**

コミットメッセージの`type`が**バージョン番号を自動決定**します。不適切なtypeの使用は、誤ったバージョン番号の自動付与につながります。

#### semantic-releaseとの関係性

| Commit Type | バージョン影響 | 例 | 使用場面 |
|-------------|--------------|-----|----------|
| `feat:` | **MINOR** ⬆️ (1.2.0 → 1.3.0) | `feat(api): ユーザー検索機能を追加` | 新機能追加時 |
| `fix:` | **PATCH** ⬆️ (1.2.0 → 1.2.1) | `fix(auth): ログイン時のタイムアウトを修正` | バグ修正時 |
| `feat!:` / `fix!:` | **MAJOR** ⬆️ (1.2.0 → 2.0.0) | `feat!: APIエンドポイントを刷新` | 破壊的変更時 |
| `BREAKING CHANGE:` | **MAJOR** ⬆️ (1.2.0 → 2.0.0) | 本文に記載 | 破壊的変更時 |
| `docs:`, `chore:`, `test:`, `refactor:`, `ci:`, `build:`, `perf:`, `style:` | **変更なし** | `docs: README更新` | リリースノートのみ記載 |

**誤ったtypeを使用した場合の影響例**:

- ❌ **間違い**: `chore: ユーザー検索機能を追加` → バージョン変更なし（本来はMINOR upであるべき）
- ❌ **間違い**: `feat: typo修正` → MINOR up（本来はPATCH upまたはdocs:であるべき）
- ❌ **間違い**: `fix: 新機能追加` → PATCH up（本来はMINOR upであるべき）
- ✅ **正しい**: `feat: ユーザー検索機能を追加` → MINOR up
- ✅ **正しい**: `fix: ログインエラーを修正` → PATCH up
- ✅ **正しい**: `docs: インストール手順を更新` → バージョン変更なし

#### Conventional Commits 形式（必須）

```
type(scope): summary

[optional body]

[optional footer(s)]
```

**許可されたtype一覧**:

- `feat`: 新機能追加（MINOR version up）
- `fix`: バグ修正（PATCH version up）
- `docs`: ドキュメントのみの変更（バージョン変更なし）
- `test`: テストの追加・修正（バージョン変更なし）
- `chore`: ビルドプロセスやツール変更（バージョン変更なし）
- `refactor`: リファクタリング（バージョン変更なし）
- `ci`: CI設定変更（バージョン変更なし）
- `build`: ビルドシステム変更（バージョン変更なし）
- `perf`: パフォーマンス改善（バージョン変更なし）
- `style`: コードスタイル変更（バージョン変更なし）

**破壊的変更の表記**:

- type に `!` を付与: `feat!:`, `fix!:`
- または本文に `BREAKING CHANGE:` を記載

**ルール**:

- `summary` は50文字以内、語尾に句読点を付けない
- `scope` はオプションだが、推奨（例: `feat(api):`, `fix(auth):`）
- 本文が必要な場合は空行を挟んで記述
- フッターには `BREAKING CHANGE:`, `Closes #123` などを記載可能

#### 検証手順（絶対必須）

**コミット前に必ず実行**:

```bash
.specify/scripts/checks/check-commits.sh --from origin/main --to HEAD
```

このスクリプトは `npx commitlint` を呼び出し、違反コミットを明示的に列挙します。

**CI検証**:

- Quality Checks ワークフローで commitlint が必ず実行される
- 違反が検出された場合、**PRマージがブロックされ、リリースが失敗する**

**違反時の修正方法**:

```bash
# 直前のコミットメッセージを修正
git commit --amend

# 複数のコミットメッセージを修正
git rebase -i HEAD~3

# 再検証
.specify/scripts/checks/check-commits.sh --from origin/main --to HEAD
```

#### 禁止事項（厳格）

- ❌ commitlintが失敗した状態でのプッシュ
- ❌ commitlintが失敗した状態でのレビュー依頼
- ❌ commitlintが失敗した状態でのプルリク作成
- ❌ 適当なtypeの選択（必ず変更内容に応じた正しいtypeを使用）
- ❌ 破壊的変更を通常のfeatやfixとして記載（必ず`!`または`BREAKING CHANGE:`を使用）

#### semantic-releaseの動作

1. mainブランチへのマージ時、すべてのコミットメッセージを解析
2. `feat:` が含まれる → MINOR version up
3. `fix:` が含まれる → PATCH version up
4. `BREAKING CHANGE:` または `!` が含まれる → MAJOR version up
5. 新しいバージョン番号でGitタグを作成
6. CHANGELOGを自動生成
7. GitHubリリースを作成
8. パッケージを公開（該当する場合）

**コミットメッセージの品質 = リリースの品質** です。慎重に記述してください。

### markdownlint準拠ドキュメント（強制）

- Markdown ファイルは commit 前に `pnpm dlx markdownlint-cli2 "**/*.md" "!node_modules" "!.git" "!.github" "!.worktrees"` を実行して lint を通過させる。対象が限定される場合でもルールに従ったグロブを使用し、必ず全ファイルを検証する。
- 各ドキュメントは MD013（行長）、MD029（リスト番号）、MD041（見出しタイトル）など既定ルールを満たすよう編集する。必要な場合のみ、`.markdownlint.json` で合意された例外設定を追加する。
- lint で検出された警告を放置した状態でのコミット・プッシュは禁止。修正が困難な場合は lint ルール変更の提案を issue に記録し、承認なしでローカル例外を入れない。
- CI の Quality Checks でも markdownlint が実行されるため、ローカルで合格しない限り PR がブロックされる。CLI での改善結果を再チェックし、ゼロ警告を確認してからレビューを依頼する。

### Spec駆動開発ライフサイクル

新機能の開発は、以下の3ステップで進めます：

1. **`/speckit.specify`**: 機能仕様書を作成 (`specs/SPEC-[UUID8桁]/spec.md`)
   - ビジネス要件とユーザーストーリーを定義
   - 「何を」「なぜ」に焦点を当てる（「どのように」は含めない）
   - SPECディレクトリを生成し、仕様を文書化
   - 本リポジトリではfeatureブランチ／Worktreeの自動作成機能は無効化されているため、現在のブランチ・作業ディレクトリのまま進める

2. **`/speckit.plan`**: 実装計画を作成 (`specs/SPEC-[UUID8桁]/plan.md`)
   - 技術スタック、アーキテクチャ、データモデルを設計
   - 憲章チェック（TDD/LLM最適化/シンプルさの原則）
   - Phase 0: 技術リサーチ (`research.md`)
   - Phase 1: 設計とコントラクト (`data-model.md`, `contracts/`, `quickstart.md`)
   - Phase 2: タスク計画 (`tasks.md`)

3. **`/speckit.tasks`**: 実行可能なタスクに分解 (`specs/SPEC-[UUID8桁]/tasks.md`)
   - Setup/Test/Core/Integration/Polishに分類
   - 並列実行可能なタスクに`[P]`マーク付与
   - 依存関係を明確化

#### Spec命名規則

- **形式**: `SPEC-[UUID8桁]`
- **UUID生成**: ランダムな英数字（小文字）8桁
  - ✅ 正しい例: `SPEC-a1b2c3d4`, `SPEC-3f8e9d2a`, `SPEC-7c4b1e5f`
  - ❌ 間違い例: `SPEC-001`, `SPEC-gameobj`, `SPEC-core-001`
- **禁止事項**:
  - 連番の使用（001, 002...）
  - 意味のある名前（gameobj, core, ui...）
  - 大文字の使用（UUID部分は小文字のみ）
- **生成方法**: `uuidgen | tr '[:upper:]' '[:lower:]' | cut -c1-8` またはオンラインUUID生成ツール

#### Worktree＆ブランチ運用（プロジェクトカスタム）

本リポジトリでは GitHub Spec Kit を導入していますが、featureブランチや Worktree を自動的に作成する機能はプロジェクト要件により無効化しています。運用方針は以下のとおりです。

- 現在割り当てられている作業環境（Codex CLI が示すカレントディレクトリ・ブランチ）のみを使用する。
- `git branch`, `git checkout`, `git switch`, `git worktree` など環境を変更するコマンドは実行しない。
- Spec Kit コマンドはドキュメント生成・更新にのみ利用し、Git ブランチや Worktree を変更しない前提で扱う。
- ブランチ／Worktree の新規作成・削除・切り替えが必要になった場合は、必ずメンテナに相談し指示を受ける。
- CI やリポジトリ管理スクリプトが環境を制御しているため、手動での操作は重大な不整合を引き起こす可能性がある。

**自動保護機構（Claude Code PreToolUse Hooks）**:

上記のルールを強制するため、Claude Code PreToolUse Hookスクリプトが自動的に以下の操作をブロックします：

- **Git操作ブロック** (`.claude/hooks/block-git-branch-ops.sh`):
  - `git checkout`, `git switch`: ブランチ切り替えを防止
  - `git worktree add`: 新しいWorktree作成を防止
  - `git branch -d/-D/-m/-M`: ブランチ削除・リネームを防止
  - 読み取り専用操作（`git branch`, `git branch --list`等）は許可

- **ディレクトリ移動ブロック** (`.claude/hooks/block-cd-command.sh`):
  - Worktree外へのcd（`cd /`, `cd ~`, `cd ../..`等）を防止
  - Worktree内のcd（`cd .`, `cd src`等）は許可

これらのHookは、コマンド実行前に自動的にチェックし、違反操作を即座にブロックします。
詳細は [specs/SPEC-dc648675/](specs/SPEC-dc648675/) を参照してください。

### TDD遵守（妥協不可）

**絶対遵守事項:**

- **Red-Green-Refactorサイクル必須**:
  1. **RED**: テストを書く → テスト失敗を確認
  2. **GREEN**: 最小限の実装でテスト合格
  3. **REFACTOR**: コードをクリーンアップ

- **禁止事項**:
  - テストなしでの実装
  - REDフェーズのスキップ（テストが失敗することを確認せずに実装）
  - 実装後のテスト作成（テストが実装より後のコミットになる）

- **Git commitの順序**:
  - テストコミットが実装コミットより先に記録される必要がある
  - 例: `feat(test): Fooのテスト追加` → `feat: Foo実装`

- **テストカテゴリと順序**:
  1. Contract tests (統合テスト) → API/インターフェース定義
  2. Integration tests → クリティカルパス100%
  3. E2E tests → 主要ユーザーワークフロー
  4. Unit tests → 個別機能、80%以上のカバレッジ

**詳細は [`memory/constitution.md`](memory/constitution.md) を参照**

### SDD (Spec-Driven Development) 規約

**すべての機能開発・要件追加は `/speckit.specify` から開始**

**新規機能開発フロー**:

1. `/speckit.specify` - ビジネス要件を定義（技術詳細なし）
   - 本リポジトリではfeatureブランチ／Worktree自動生成を停止しているため、現在のブランチと作業ディレクトリのまま仕様ファイルを生成・更新する。
2. `/speckit.plan` - 技術設計を作成（憲章チェック必須）
   - 現在の作業ディレクトリで実行し、Gitの状態を変えない。
3. `/speckit.tasks` - 実行可能タスクに分解
   - 実装実行は `/speckit.implement` で補助的に利用可能
   - 環境を移動せずタスクを細分化する。
4. タスク実行（TDDサイクル厳守）
   - 割り当て済みブランチ上で実装し、コミットを積む。ブランチ操作は禁止。
5. 完了時はメンテナまたはCIが用意した手順に従う。`finish-feature.sh` を含むブランチ／Worktree操作系スクリプトを自己判断で実行しない。

**既存機能のSpec化フロー**:

1. 作業ディレクトリを移動せず、現在の環境で `/speckit.specify` を実行して実装済み機能のビジネス要件を文書化する。
2. `/speckit.plan` で必要に応じて技術設計を追記する。
3. 既存実装とSpecの整合性を確認する。
4. 完了報告やGit操作が必要な場合はメンテナの指示を待つ。自己判断でブランチやWorktreeを操作しない。

**Spec作成原則**:

- ビジネス価値とユーザーストーリーに焦点
- 「何を」「なぜ」のみ記述（「どのように」は禁止）
- 非技術者が理解できる言葉で記述
- テスト可能で曖昧さのない要件

**憲章準拠**:

- すべての実装は [`memory/constitution.md`](memory/constitution.md) に準拠
- TDD、ハンドラーアーキテクチャ、LLM最適化は妥協不可

## コミュニケーションガイドライン

- 回答は必ず日本語

## ドキュメント管理

- ドキュメントはREADME.md/README.ja.mdに集約する

## コードクオリティガイドライン

- マークダウンファイルはmarkdownlintでエラー及び警告がない状態にする
- コミットログはcommitlintに対応する

## 開発ガイドライン

- 既存のファイルのメンテナンスを無視して、新規ファイルばかり作成するのは禁止。既存ファイルを改修することを優先する。

## ドキュメント作成ガイドライン

- README.mdには設計などは書いてはいけない。プロジェクトの説明やディレクトリ構成などの説明のみに徹底する。設計などは、適切なファイルへのリンクを書く。
