# Lessons Learned

ユーザーからの修正指示や作業中に学んだ教訓を記録し、再発を防止する。
セッション開始時にこのファイルを確認し、過去の教訓を踏まえて作業すること。

## 記録ルール

- 修正を受けたら、原因・正しい対応・再発防止ルールの3点を記録する
- 同じカテゴリの教訓はまとめて更新する
- 解消済み・陳腐化した教訓は削除する

## 教訓一覧

### tokio RwLock の write guard を .await をまたいで保持しない

- **事象**: `check_and_maybe_download` で `state.write().await` の write guard を名前付き変数として保持したまま `ensure_payload_ready().await` を呼び出し、内部で `state.read().await` を試みてデッドロック
- **原因**: tokio の `RwLock` はリエントラントではないため、同一タスクが write lock を保持したまま read lock を取得しようとすると永久にブロックされる。Rust の NLL はボローチェッカーの分析にのみ影響し、Drop のタイミング（スコープ末尾）は変えない
- **再発防止ルール**: `RwLockWriteGuard` / `RwLockReadGuard` は必ずスコープブロック `{ }` で囲み、`.await` をまたがせない。名前付きガード変数がある場合は `.await` の前に `drop()` するかブロックで囲む
- **次回チェック方法**: `state.write().await` を名前付き変数に束縛している箇所で、同一スコープ内に `.await` がないか grep で確認

### エンドポイントのヘルスチェック Online は推論成功を意味しない

- **事象**: Claude Code から `/v1/messages` に `openai/gpt-oss-20b` で問い合わせたところ、ダッシュボード上は Online 表示の LM Studio エンドポイントで 2 分ちょうど（= 120 s）で `502 OpenAI-compatible upstream request failed` が連発。成功率は 2.5% まで下がっていた
- **原因**: `/v1/models` への軽量ヘルスチェックは即応するが、20B モデルの初回 `/v1/chat/completions` は VRAM ロードだけで既定 `inference_timeout_secs=120` を超える。reqwest の `send()` がタイムアウトし `forward_to_endpoint` が Err を返す
- **再発防止ルール**: 502 系の症状を見たら、まずダッシュボードの Endpoints 成功率と Avg Response Time、該当 History の Duration / Error を確認する。ヘルスチェック通過だけでは推論可否を保証しないので、`inference_timeout_secs` をモデル規模に合わせて個別に設定する（目安: 20B 級は 600 s 以上、クラウドプロキシは 120 s でよい）
- **次回チェック方法**: Dashboard → Endpoints の該当行の成功率カラム、History → 該当レコードの Duration が `inference_timeout_secs` と一致していないかを確認する

### Anthropic 502 レスポンスの message はハードコードで、実エラーは握りつぶされる

- **事象**: クライアントには `OpenAI-compatible upstream request failed` としか返らず、タイムアウトなのか接続拒否なのか DNS なのか区別できない。原因特定が大幅に遅れた
- **原因**: `llmlb/src/api/anthropic.rs:500-504` で `forward_to_endpoint` の `Err` を固定文字列に差し替えている。`proxy.rs:394-401` の `tracing::error!` には reqwest の実エラーが出ているが、HTTP 応答には反映されない。`/v1/chat/completions` 側の同種パスも同じ構造
- **再発防止ルール**: 502 受領時はクライアント出力だけで判断せず、`Dashboard → History → Request Details → Error` フィールド、もしくは llmlb の標準エラー出力（tracing）を必ず確認する。中期的にはエラーメッセージの透過化（`LbError` の実メッセージを Anthropic 応答に流す）を別 SPEC で対応する
- **次回チェック方法**: `GET /api/request_history` または Dashboard の該当レコードの Error フィールド、llmlb プロセスの標準出力で `Failed to forward request to endpoint` の直近ログを確認

### 一時 git リポジトリを使うテストは git フックの環境変数を必ず解除する

- **事象**: `check-migration-versions.sh` の bats テストが pre-push フック（`make quality-checks`）内で実行された際、一時ディレクトリで行ったはずの `git init` / `git commit` / `git config user.*` が実リポジトリに作用し、作業ブランチに全ファイル削除コミットが積まれ、共有 git config に `[user] test` が追記された。その間に別 worktree の agent のコミット author が `test` になった
- **原因**: git フックは `GIT_DIR` などを export した状態で子プロセスを起動する。`GIT_DIR` が設定されていると、`cd` で別ディレクトリに移っても git は実リポジトリを操作する。直接 `bats` を実行したときは環境変数がないため再現しない
- **再発防止ルール**: 一時リポジトリを作る bats テストは必ず `tests/checks/helpers/git-sandbox.bash` の `git_sandbox_init` を経由する（`git rev-parse --local-env-vars` の unset、toplevel の assert、author は環境変数で指定し `git config` に書き込まない）。隔離そのものは `tests/checks/test-git-sandbox.bats` が囮リポジトリで回帰検証する
- **次回チェック方法**: テスト追加後は直接実行だけでなく `git push`（pre-push 経由）でも実行し、終了後に `git log -1`、`git status`、`git config --local --get user.email` が変化していないことを確認する

### localhost の MockServer は同一ホストの別プロセスからも要求を受ける

- **事象**: `test_health_check_falls_back_to_v1_models_when_api_health_fails_for_xllm` が、`--test-threads=1` の逐次実行にもかかわらず `GET /api/health` の呼び出し回数 2 で失敗した（Issue #748）。同じ時間帯に Playwright E2E の `llmlb serve` が並走していた
- **原因**: E2E のモックエンドポイントは `127.0.0.1:0` の一時ポートで xLLM として登録され、llmlb のヘルスチェッカーが 30 秒ごとに `GET /api/health` を送る。解放されたポートを wiremock の MockServer（同じく `127.0.0.1:0`）が再利用すると、別プロセスの要求がテストのモックに到達し、パスだけで照合する回数カウントに混入する
- **再発防止ルール**: 呼び出し回数を厳密に検証するテストでは、エンドポイントの `base_url` とモックのパスをテストごとに一意なベースパス（UUID）配下へ置き、外部からの要求を照合対象から外す。同時に、外部要求を注入しても結果が変わらないことをテストで保証する
- **次回チェック方法**: `path("/api/health")` や `path("/v1/models")` を回数カウントや `.expect(n)` と組み合わせている箇所を grep し、ベースパスで隔離されているか確認する
