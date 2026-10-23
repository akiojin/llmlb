# モデルカタログ運用ポリシー

`/v1/models` レスポンスにおける `id` / `canonical_name` / `aliases` / `max_tokens` /
`supported_apis` 等の挙動と、`llmlb/src/models/mapping.rs` のメンテナンス指針を定める。

本書は SPEC #575 「OpenAI互換APIゲートウェイ」US-001 の補正対応で発見された
メタデータ品質課題（B-1〜B-5、G-1〜G-7）を踏まえた運用方針である。

## エイリアス命名規則

`BUILTIN_MAPPINGS` (`llmlb/src/models/mapping.rs`) の `aliases` には、各エンドポイント
タイプ（Ollama / LM Studio / xLLM など）が利用するモデル名をすべて列挙する。

ルール:

- canonical 名は HuggingFace 上で**実在するリポジトリ ID** を採用する（例:
  `zai-org/GLM-4.7-Flash`、`openai/gpt-oss-20b`）。組織名が変わったモデルでは、
  旧 org の表記を alias として残す（例: `THUDM/glm-4.7-flash` → alias）。
  canonical の ID を差し替えた場合も同様に、旧 ID を alias の**末尾**へ残す（先頭の alias が
  エンドポイントへ送る名前になるため）。
- パラメータ数による canonical 対象の除外は行わず、マルチ GPU 環境向けの大型モデルも扱う（Issue #884）。
- alias 名は **各エンドポイントタイプ固有の命名形式** に従う。
  - Ollama: `family:tag`（例: `gpt-oss:20b`、`gemma4:e4b`）
  - LM Studio: HuggingFace 形式（例: `openai/gpt-oss-20b`）
- 新規 mapping では `:latest` のような **可変タグを原則登録しない**。Issue #643 の
  Gemma 4 mapping には `gemma4:latest` を登録せず、SKU 固定タグを使う。一方、
  Qwen / GLM / Nomic 等の既存 `:latest` alias は後方互換性のため残り得る。
- 量子化サフィックス（`:Q4_K_M` など）は現状 ID に同居しているケースがあるが、
  正規化方針は別 SPEC で扱う（後述「量子化サフィックス方針」）。

## canonical_name の解決とフォールバック

`CanonicalResolution::canonical_for(model_key)` は次の順で解決する。

1. `model_key` 自身が canonical テーブルに登録 → そのまま返す。
2. `model_to_canonical` 逆引きで対応する canonical を返す。
3. いずれも未登録 → **`model_key` 自身を canonical として返す**（self-fallback）。

「mapping に登録があるか？」の判定が必要な場合は `CanonicalResolution::is_known()`
を併用する。`canonical_for` は `String` を必ず返す（`null` を返さない）。

`/v1/models` と `/api/dashboard/models` の `is_canonical` は、その行の `id` 自身が
canonical テーブルに登録された canonical 名である場合のみ `true` になる。self-fallback
で `canonical_name` に `id` が入っているだけの未登録モデルは `false` とし、ダッシュボードは
このフラグで canonical バッジを表示する（Issue #722）。

## max_tokens フォールバック（B-1 / G-7）

`/v1/models` の `max_tokens` は次の優先順で解決する（`llmlb/src/models/mapping.rs:resolve_max_tokens`）。

1. エンドポイント側 `/v1/models` が申告した `max_tokens`。
2. `KNOWN_CONTEXT_LENGTHS` テーブルから canonical 一致で取得。
3. いずれにも該当なし → `null`。

`KNOWN_CONTEXT_LENGTHS` には公開情報（HuggingFace モデルカード等）から確認できた
モデルの context length のみを登録する。バリエーションごとに異なる値が公称される
モデル（量子化バリエーションで差がある等）は登録しない。

Issue #643 で確認した fallback 値は次のとおり。

| canonical | context length (tokens) |
|---|---:|
| `google/gemma-4-E2B-it` | 131072 |
| `google/gemma-4-E4B-it` | 131072 |
| `google/gemma-4-26B-A4B` | 262144 |
| `google/gemma-4-26B-A4B-it` | 262144 |
| `nvidia/NVIDIA-Nemotron-3-Nano-30B-A3B-BF16` | 1048576 |
| `nvidia/NVIDIA-Nemotron-3-Nano-4B-BF16` | 262144 |
| `Qwen/Qwen3-Coder-Next` | 262144 |
| `zai-org/GLM-4.7-Flash` | 202752 |

Nemotron 30B の 1048576 はモデルカードの公称最大 1M tokens であり、標準設定の値と
異なる場合がある。実際の応答では前述のとおりエンドポイント申告値を優先する。

新しいモデルを追加する際は出典（HF README / 公式リリースノート等）を PR 説明に
記載する。

## 検証済み SKU と派生バリエーション（G-1 / G-2 / G-5 / C-3）

ggml-org のように community 配布版のリポジトリは、独自 SKU 接頭辞（`E2B`、`E4B`
等）を付ける場合がある。Issue #643 では一次情報と照合し、同一 SKU の GGUF
再配布を次の official canonical へ明示的に集約した。サイズ違い、および base と
instruction-tuned（`-it`）は別 SKU として扱う。

| official canonical | 検証済みの対応・備考 |
|---|---|
| `google/gemma-4-E2B-it` | `ggml-org/gemma-4-E2B-it-GGUF` |
| `google/gemma-4-E4B-it` | `ggml-org/gemma-4-E4B-it-GGUF` |
| `google/gemma-4-26B-A4B` | Google 公式 base。対応する Ollama alias は割り当てない |
| `google/gemma-4-26B-A4B-it` | `ggml-org/gemma-4-26B-A4B-it-GGUF` |
| `nvidia/NVIDIA-Nemotron-3-Nano-30B-A3B-BF16` | NVIDIA 公式 30B-A3B BF16 |
| `nvidia/NVIDIA-Nemotron-3-Nano-4B-BF16` | NVIDIA 公式 4B BF16 |
| `Qwen/Qwen3-Coder-Next` | Qwen 公式 |
| `zai-org/GLM-4.7-Flash` | Z.ai 公式。旧 `THUDM/glm-4.7-flash` は alias |

Gemma 4 の Ollama runtime 名は、SKU を一意にする固定タグだけを利用する。

- E2B instruction-tuned: `gemma4:e2b`
- E4B instruction-tuned: outbound は固定タグ `gemma4:e4b` を優先する。既存の
  bare `gemma4` は互換入力専用の legacy alias として受理する
- 26B instruction-tuned: `gemma4:26b`
- 26B base: Ollama alias なし

`gemma4:latest` は参照先が将来変わり得るため、alias に含めない。

## canonical テーブルの更新手順（Issue #776）

### 新リリースと話題モデルの週次検知（Issue #885）

`HF Model Discovery`（`.github/workflows/model-discovery.yml`）が毎週月曜 03:00 UTC
（日本時間 12:00）に実行される。GitHub Actions の `workflow_dispatch` でも再実行できる。
定期実行には workflow がデフォルトブランチへリリースされている必要がある。
検査する対応表と設定は `develop` から取得する。

公開元リストと件数は `scripts/models/hf-discovery-config.json` だけで管理する。
初期設定は Qwen、zai-org、google、nvidia、meta-llama、moonshotai、mistralai、
deepseek-ai、openai、microsoft。公開元別に `createdAt` 降順でページングし、直近14日
のモデルを取得する。週次実行の遅延や一時失敗を吸収するため、期間を2週間にしている。
話題度の代理指標は HF API の **`trendingScore` 降順上位100件**。X の直接監視はしない。
順位はテキスト生成以外も含む全モデル内の順位で、トレンド側は公開日で制限しない。
API の属性は [HF 公式リファレンス][hf-list-models] を参照。

[hf-list-models]: https://huggingface.co/docs/huggingface_hub/package_reference/hf_api#huggingface_hub.HfApi.list_models

候補は `BUILTIN_MAPPINGS` の canonical / alias に未登録（大文字小文字を無視した
完全一致）、かつ `text-generation` / `text2text-generation` / `image-text-to-text` の
モデルに限る。規模は `safetensors.total` または `gguf.total` の**総パラメータ数**が
120B 未満であることを要求する。両形式の総数がある場合は大きい方を採用し、
小さな補助重みだけで GGUF 本体の規模を過小評価しない。
120B ちょうど、規模不明、公開日不明は除外する。
モデル名や MoE の active parameter 数だけから規模を推測しない。規模不明の ID は
レポートの `unverified_size` に残るため、必要なら別途メタデータを確認する。

候補があると marker `<!-- llmlb:hf-model-discovery -->` または固定タイトル
`[model-discovery] Hugging Face 新モデル候補` の open Issue を全ページから探す。
なければ1件作成し、あれば未記載モデルだけコメントに追記する。各行にモデル ID、
公開日（HF リポジトリ作成日時の UTC 日付）、トレンド順位、総パラメータ数を記載する。
同一 workflow は concurrency で直列化し、コメント済み ID の再通知も抑制する。
候補の確認が終わったら集約 Issue を閉じる。採用する場合は以下の通常手順に従い、
実際のエンドポイント ID、Ollama / LM Studio のタグ実在を確認してから対応表を変更する。
**検知ジョブは対応表を自動変更しない。**

HF API の HTTP エラー、タイムアウト、JSON/応答形式の異常は警告にして exit 0 で終える。
`status: incomplete` と失敗した取得元を出力し、取得済み候補は保持する。
`incomplete` かつ候補ゼロでも「新モデルなし」を意味しない。Actions の警告と artifact
`hf-model-discovery` 内の `discovery-report.json` を確認し、回復後に手動再実行する。
候補ゼロを確定できるのは `status: complete` の場合だけである。設定・対応表の破損や
GitHub Issue 操作の失敗はジョブを失敗させ、無言で通知を失わない。

HF の公開 API は認証なしで利用する。レート制限対策として repository secret
`HF_TOKEN` を任意で設定できる。Issue 操作には `GITHUB_TOKEN` の `issues: write`
だけを使う。ローカルでは次のコマンドで通知なしの確認とオフラインテストができる。

```bash
python3 scripts/models/discover_hf_models.py --output /tmp/hf-discovery.json
python3 -m unittest discover -s tests/models -v
make test-checks
```

`--publish` は実際に Issue を作成・追記するため、ローカルの確認には付けない。

### 既存エントリの鮮度検査

`BUILTIN_MAPPINGS` は人手で編集する静的テーブルで、自動では追従しない。各エントリの
`last_verified`（Hugging Face 上で canonical の実在を最後に確認した日）を
`make mapping-freshness` が検査し、**180 日**を超えたエントリが 1 件でもあると失敗する。
この検査は `make quality-checks` と、CI の必須チェック `Commit Message Lint` ジョブの
両方で実行される。ネットワークには依存しない（時計だけを見る）。

検査が失敗したとき、またはモデルを追加・是正したいときは次の順で作業する。

### 1. どこを見るか

- 対象は `llmlb/src/models/mapping.rs` の `BUILTIN_MAPPINGS`。`make mapping-freshness`
  の出力が、期限切れのエントリと経過日数を名指しする。
- 実在確認は Hugging Face Models API を使う。認証なしの場合、`200` は実在、`401` /
  `404` は不存在（または非公開）を意味する。

  ```bash
  curl -s -o /dev/null -w '%{http_code}\n' https://huggingface.co/api/models/<org>/<repo>
  ```

- 稼働中エンドポイントが報告する ID は、各エンドポイントの `/v1/models`（LM Studio は
  `/api/v0/models` で publisher も取得可）で確認する。

### 2. 何を根拠にするか

- **追加**: 稼働中エンドポイントが報告して解決に失敗したモデル ID、または承認済み Issue で
  指定され一次情報で実在を確認した公開モデルを対象とする。「新しい版が出たはず」という推測では追加しない。
- **canonical の選定**: Hugging Face 上で実在を確認した一次配布元のリポジトリ ID。
  エンドポイントの実体が再配布リポジトリ（GGUF / MLX 変換等）の場合は、その
  `base_model` が指す配布元を canonical にする。
- **alias**: 実際のエンドポイント応答、または公式リポジトリ名。確認できていないランタイムの
  alias は登録しない（例: LM Studio でしか確認していないモデルに Ollama alias を付けない）。
- **改名・廃止**: 実在確認で `401` / `404` になった canonical は、Models API の検索
  （`/api/models?author=<org>&search=<name>`）で現行の ID を特定して差し替える。
  後継が特定できない場合はエントリを削除する。

### 3. どう更新するか

- 追加・差し替えは、先に `llmlb/src/models/mapping/tests.rs` へ失敗するテストを書く。
- canonical を差し替えたら、旧 ID を alias の末尾へ残し、`KNOWN_CONTEXT_LENGTHS` のキーも
  新しい canonical に合わせる。
- 根拠（API 応答、エンドポイント応答）を PR 本文に記載する。

### 4. 最終確認日をどう更新するか

- 手順 1 の実在確認で `200` を得たエントリの `last_verified` を、**確認した当日の日付**
  （UTC, `YYYY-MM-DD`）へ更新する。確認していないエントリの日付は更新しない。
- 日付だけを機械的に書き換えて検査を通すことは禁止する。`last_verified` は「その日に
  実在を確認した」という記録である。
- 更新後に `make mapping-freshness` が成功することを確認する。

### 2026-09-30 の確認結果

Models API で当時の 18 件を確認し、16 件が実在、2 件が不存在だった。不存在の 2 件は
実在する ID へ差し替え、旧 ID は alias として残した。

| 旧 canonical（不存在） | 現行 canonical |
|---|---|
| `Qwen/Qwen3-30B` | `Qwen/Qwen3-30B-A3B` |
| `nvidia/nemotron-3-super-120b-a12b` | `nvidia/NVIDIA-Nemotron-3-Super-120B-A12B-BF16` |

稼働中の LM Studio が報告し解決に失敗していた 2 件を追加した。

| LM Studio の ID | canonical |
|---|---|
| `qwen/qwen3.8-27b` | `Qwen/Qwen3.8-27B` |
| `qwen-image-edit-rapid-aio` | `Phr00t/Qwen-Image-Edit-Rapid-AIO` |

### 2026-10-13 の追従結果（Issue #884）

以下の 8 canonical は Hugging Face Models API で `200` と ID の一致を確認し、
`last_verified` に確認日（UTC）を記録した。Kimi は公式 `moonshotai` の
[公開モデル一覧](https://huggingface.co/api/models?author=moonshotai&sort=createdAt&direction=-1&limit=100&full=true)
を作成日時降順で照合し、最新のオープンウェイト `moonshotai/Kimi-K3` を選んだ。
K3 はマルチモーダルだが、公式 README にテキスト入力の生成例と vLLM / SGLang のローカル実行手順がある。

| canonical | context length | 一次情報 |
|---|---:|---|
| `zai-org/GLM-5.3-Flash` | 1048576 | [config](https://huggingface.co/zai-org/GLM-5.3-Flash/blob/main/config.json) |
| `Qwen/Qwen3.8-Flash-Next` | 262144 | [config](https://huggingface.co/Qwen/Qwen3.8-Flash-Next/blob/main/config.json) |
| `Qwen/Qwen3.6-35B-A3B` | 262144 | [config](https://huggingface.co/Qwen/Qwen3.6-35B-A3B/blob/main/config.json) |
| `google/gemma-4-31B-it` | 262144 | [config](https://huggingface.co/google/gemma-4-31B-it/blob/main/config.json) |
| `google/gemma-4-12B-it` | 262144 | [config](https://huggingface.co/google/gemma-4-12B-it/blob/main/config.json) |
| `nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16` | 262144 | [config](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16/blob/main/config.json) |
| `moonshotai/Kimi-K3` | 1048576 | [config](https://huggingface.co/moonshotai/Kimi-K3/blob/main/config.json) |
| `meta-llama/Llama-4-Scout-17B-16E-Instruct` | 10485760 | [Meta 公式 SKU 定義](https://github.com/meta-llama/llama-models/blob/main/models/sku_types.py) |
| `Qwen/Qwen3.8-27B`（既存） | 262144 | [config](https://huggingface.co/Qwen/Qwen3.8-27B/blob/main/config.json) |
| `Qwen/Qwen3-30B-A3B`（既存） | 40960 | [config](https://huggingface.co/Qwen/Qwen3-30B-A3B/blob/main/config.json) |

Scout の HF config は gated（未認証で `401`）のため、Meta 公式の Instruct SKU の
`max_seq_length` を用いた。他は config の `max_position_embeddings`（マルチモーダルでは
`text_config` 配下）を採用した。Nemotron の拡張時 1M や Qwen3-30B-A3B の YaRN 値と区別し、
実行環境の申告値があればそちらを優先する。

登録した runtime 名と出典は次のとおり。Ollama は `latest` を使わず、確認できた明示タグを使う。
LM Studio は公式 catalog のキー、または公式配布 GGUF のリポジトリ名を使い、
配布元の `base_model` または README の `Original model` リンクと canonical の対応も確認した。

| モデル | Ollama の確認済みタグ | LM Studio の確認済み名 |
|---|---|---|
| GLM-5.3-Flash | [`glm-5.3-flash:cloud`](https://ollama.com/library/glm-5.3-flash/tags) | なし（ローカル runtime 名未確認） |
| Qwen3.8-Flash-Next | [`qwen3.8-flash-next:125b-a6b-q4_K_M`](https://ollama.com/library/qwen3.8-flash-next/tags) | [`qwen/qwen3.8-flash-next`](https://lmstudio.ai/models/qwen/qwen3.8-flash-next) |
| Qwen3.6-35B-A3B | [`qwen3.6:35b-a3b-q4_K_M`](https://ollama.com/library/qwen3.6/tags) | [`qwen/qwen3.6-35b-a3b`](https://lmstudio.ai/models/qwen/qwen3.6-35b-a3b) |
| Gemma 4 31B IT | [`gemma4:31b-it-q4_K_M`](https://ollama.com/library/gemma4/tags) | [`lmstudio-community/gemma-4-31B-it-GGUF`](https://huggingface.co/lmstudio-community/gemma-4-31B-it-GGUF) |
| Gemma 4 12B IT | [`gemma4:12b-it-q4_K_M`](https://ollama.com/library/gemma4/tags) | [`lmstudio-community/gemma-4-12B-it-GGUF`](https://huggingface.co/lmstudio-community/gemma-4-12B-it-GGUF) |
| Nemotron 3.5 Lightning | [`nemotron-3.5-lightning:30b-a3b-q4_K_M`](https://ollama.com/library/nemotron-3.5-lightning/tags) | [`lmstudio-community/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-GGUF`](https://huggingface.co/lmstudio-community/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-GGUF) |
| Kimi K3 | [`kimi-k3:cloud`](https://ollama.com/library/kimi-k3/tags) | なし（ローカル runtime 名未確認） |
| Llama 4 Scout | [`llama4:17b-scout-16e-instruct-q4_K_M`](https://ollama.com/library/llama4/tags) | [`lmstudio-community/Llama-4-Scout-17B-16E-Instruct-GGUF`](https://huggingface.co/lmstudio-community/Llama-4-Scout-17B-16E-Instruct-GGUF) |

GLM / Kimi の `:cloud` は Ollama が提供するクラウド実行タグで、ローカル重みのタグではない。
LM Studio catalog の bare slug や未確認のローカルタグを別名として推測登録しない。
Gemma の catalog 名は `-it` が省略されて base の HF ID と衝突するため、IT を明示する配布リポジトリ名を使う。

### 2026-10-19 の候補確認結果（Issue #934）

`origin/develop` と作業 HEAD が同一の状態から、通知された48件すべての
`https://huggingface.co/api/models/<repo>` が HTTP 200 と ID 一致を返すことを確認した。
次の7 canonicalを採用し、`last_verified` は確認日を記録した。
推論 API や名前解決アルゴリズムの変更は行わない。

| canonical | context length と出典 | Ollama の確認済み固定名 | LM Studio の確認済み名 |
|---|---|---|---|
| `openbmb/MiniCPM-V-4.7-35B-A3B` | [config `text_config.max_position_embeddings`](https://huggingface.co/openbmb/MiniCPM-V-4.7-35B-A3B/blob/main/config.json): 262144 | [`openbmb/minicpm-v4.7:35b-a3b-q4_K_M`](https://ollama.com/openbmb/minicpm-v4.7/tags) | なし（LM Studio名未確認） |
| `openbmb/MiniCPM-V-4.7-1B` | [config `text_config.max_position_embeddings`](https://huggingface.co/openbmb/MiniCPM-V-4.7-1B/blob/main/config.json): 262144 | [`openbmb/minicpm-v4.7:1b-q4_K_M`](https://ollama.com/openbmb/minicpm-v4.7/tags) | なし（LM Studio名未確認） |
| `LiquidAI/LFM2.5-24B-A2B` | [config `max_position_embeddings`](https://huggingface.co/LiquidAI/LFM2.5-24B-A2B/blob/main/config.json): 131072 | [`lfm2.5:24b-a2b-q4_K_M`](https://ollama.com/library/lfm2.5/tags) | [`LiquidAI/LFM2.5-24B-A2B-GGUF`](https://huggingface.co/LiquidAI/LFM2.5-24B-A2B-GGUF) |
| `nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-BF16` | [公式モデルカード](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-BF16): 262144 | [`nemotron-3.5-bolt:74b-a9b-iq2`](https://ollama.com/library/nemotron-3.5-bolt/tags) | なし（LM Studio名未確認） |
| `nvidia/NVIDIA-Nemotron-3.5-Super-VL-120B-A12B-BF16` | [config `llm_config.max_position_embeddings`](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Super-VL-120B-A12B-BF16/blob/main/config.json): 1048576 | [`nemotron-3.5-super:120b-a12b-nvfp4`](https://ollama.com/library/nemotron-3.5-super/tags) | [`nvidia/nemotron-3.5-super-vl`](https://lmstudio.ai/models/nvidia/nemotron-3.5-super-vl) |
| `JetBrains/Mellum2.1-12B-A2.5B-Thinking` | [config `max_position_embeddings`](https://huggingface.co/JetBrains/Mellum2.1-12B-A2.5B-Thinking/blob/main/config.json): 131072 | [`hf.co/JetBrains/Mellum2.1-12B-A2.5B-Thinking-GGUF:Q4_K_M`](https://huggingface.co/JetBrains/Mellum2.1-12B-A2.5B-Thinking-GGUF#run-with-ollama) | [`JetBrains/Mellum2.1-12B-A2.5B-Thinking-GGUF`](https://huggingface.co/JetBrains/Mellum2.1-12B-A2.5B-Thinking-GGUF) |
| `meta-llama/Llama-3.1-8B-Instruct` | [Meta公式モデルカード](https://huggingface.co/meta-llama/Llama-3.1-8B-Instruct): 131072 | [`llama3.1:8b-instruct-q4_K_M`](https://ollama.com/library/llama3.1/tags) | [`meta/meta-llama-3.1-8b`](https://lmstudio.ai/models/meta/meta-llama-3.1-8b) |

LM Studio は公式 catalog 名、または上表の公開 GGUF 配布リポジトリ名を使う。
Super VL の catalog が指す
[`lmstudio-community/NVIDIA-Nemotron-3.5-Super-VL-120B-A12B-GGUF`](https://huggingface.co/lmstudio-community/NVIDIA-Nemotron-3.5-Super-VL-120B-A12B-GGUF) と、
Llama の catalog が指す
[`lmstudio-community/Meta-Llama-3.1-8B-Instruct-GGUF`](https://huggingface.co/lmstudio-community/Meta-Llama-3.1-8B-Instruct-GGUF)
も同一 SKU の alias として登録する。
Llama の旧 `Meta-Llama-3.1-8B-Instruct` 表記は HF API の redirect 先が上表の ID であることを確認した。
[LFM2.5 の著者README](https://huggingface.co/LiquidAI/LFM2.5-24B-A2B#-inference) は
LM Studio対応と[導入手順](https://docs.liquid.ai/deployment/on-device/lm-studio)を明示し、同READMEが公式GGUF配布を案内する。
Mellum の GGUF README は Ollama / LM Studio 対応と上記コマンドを明示している。
MiniCPM 2件とBoltは公式GGUFの実在だけではLM Studioでのruntime名の裏づけとして
不十分なため、Ollamaだけを登録する。LM Studio名は未確認のまま空に保つ。
MiniCPM 1B / 35B、LFM2 / LFM2.5、Mellum base / Thinking は別 SKU とする。

Bolt は Super VL から圧縮・再学習された別モデルなので、Super VL に統合しない。
Bolt の24GB版は公式カードの `base_model_relation: quantized` と配布リンクから
Bolt BF16 に対応付ける。Super VL は120Bモデルだが、候補通知の120B未満という抽出条件と
静的対応表の採用範囲は別であり、大型モデルも扱う既存方針に従う。
モデルカード上のコンテキスト長よりエンドポイントの申告値を優先する。

#### 全候補の判定

以下の ID はすべて Hugging Face の同名リポジトリを指す。
「原モデル採用」は新しい canonical へ対応する配布を確認したことを意味し、
その ID 自体が未確認 runtime の alias として登録されたことは意味しない。
保留は今回の調査の結論であり、モデルの不存在や将来の不採用を意味しない。
fine-tune を `base_model` だけから原モデルへ統合することはしない。

| HF リポジトリ ID | 判定・根拠 |
|---|---|
| `openbmb/MiniCPM-V-4.7-35B-A3B` | 採用。公式 Ollama 35B 固定タグと GGUF 配布あり |
| `jialinyyzz/humanizer` | 保留。Gemma派生の別モデルで、対象runtime名未確認 |
| `LiquidAI/LFM2.5-24B-A2B` | 採用。公式 Ollama 固定タグと GGUF 配布あり |
| `baidu/Unlimited-OCR` | 保留。専用OCRアーキテクチャで、対象runtime名未確認 |
| `openbmb/MiniCPM-V-4.7-1B` | 採用。公式 Ollama 1B 固定タグと GGUF 配布あり |
| `nvidia/Qwen3.8-Flash-Next-FP4-Local` | 保留。既存QwenのModel Optimizer配布だが、対象runtime名未確認 |
| `LiquidAI/LFM2.5-24B-A2B-GGUF` | 採用。公式量子化配布をLFM2.5 canonicalのLM aliasへ統合 |
| `tencent/Hy-Memory-30B-A3B` | 保留。独自hy_v3アーキテクチャで、対象runtime名未確認 |
| `ConwayResearch/Underdog-Saluki-27B-1.0` | 保留。再学習された派生モデルで、対象runtime名未確認 |
| `openbmb/MiniCPM-V-4.7-35B-A3B-gguf` | 原モデル採用。公式35B量子化だが、LM Studio名は未確認で登録しない |
| `nerkyor/Qwen3.8-27B-Coder390-EfficientThink-Opus5.5-GPT6Astra-Grok4.7-DSV4Pro-K3-SFT-RLOO-MTP-DFlash2` | 保留。SFT/RL派生を元Qwenに統合せず、対象runtime名未確認 |
| `nvidia/NVIDIA-Nemotron-3.5-Super-VL-120B-A12B-NVFP4` | 原モデル採用。公式quantized関係からSuper VL BF16をcanonicalに選定 |
| `DavidAU/Qwen3.8-27B-TURBO-Fable-Cold-Fusion-735-882-Heretic-Uncensored-NEO-CODER-MAX-MTP-GGUF` | 保留。派生モデルで、対象runtime名未確認 |
| `Cloudflare/clef` | 保留。Ollama公式説明では通常chatでなく専用 `/v1/systemone` のdecision model |
| `nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB` | 原モデル採用。公式quantized関係からBolt BF16をcanonicalに選定 |
| `nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-BF16` | 保留。公式手順に専用llama.cpp-doryが必要で、Ollama/LM名未確認 |
| `LiquidAI/d1-3B` | 保留。独自実行方式を要する派生モデルで、対象runtime名未確認 |
| `HauhauCS/Qwen3.8-27B-Uncensored-HauhauCS-Aggressive-MTP-GGUF` | 保留。uncensored派生を元Qwenに統合せず、対象runtime名未確認 |
| `nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB-GGUF` | 原モデル採用。公式量子化の関係は確認したが、LM Studio名は未確認で登録しない |
| `prism-ml/Ternary-Bonsai-2-9B-gguf` | 保留。独自ternary実行方式で、対象runtime名未確認 |
| `ISTA-DASLab/Qwen3.8-27B-GSQ-RCO-GGUF` | 保留。既存Qwenの特殊量子化配布だが、対象runtime名未確認 |
| `orcarouter/OrcaCyber-Nano-1.0` | 保留。専用派生モデルで、対象runtime名未確認 |
| `openbmb/MiniCPM-V-4.7-1B-gguf` | 原モデル採用。公式1B量子化だが、LM Studio名は未確認で登録しない |
| `unsloth/LFM2.5-24B-A2B-GGUF` | 原モデル採用。第三者配布のruntime名は追加せず、公式配布名を優先 |
| `prism-ml/Ternary-Bonsai-2-27B-gguf` | 保留。独自ternary実行方式で、対象runtime名未確認 |
| `huihui-ai/Huihui-Qwen3.8-27B-abliterated-GGUF` | 保留。abliterated派生を元Qwenに統合せず、対象runtime名未確認 |
| `unsloth/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-GGUF` | 原モデル採用。第三者配布のruntime名は追加せず、公式配布名を優先 |
| `DavidAU/Qwen3.8-27B-Turbo-Brilliance-50X-Reasoning-Instruct-NEO-MAX-MTP-GGUF` | 保留。派生モデルで、対象runtime名未確認 |
| `meta-llama/Llama-3.1-8B-Instruct` | 採用。Ollama固定タグとLM公式catalogあり |
| `Cloudflare/clef-flash` | 保留。clef同様、専用 `/v1/systemone` のdecision model |
| `DavidAU/Qwen3.5-9B-The-Defiant-Fable-Uncensored-Heretic-NEO-IMATRIX-MAX-MTP-GGUF` | 保留。派生モデルで、対象runtime名未確認 |
| `JetBrains/Mellum2.1-12B-A2.5B-Thinking` | 採用。公式GGUF READMEがOllama固定名とLM対応を明示 |
| `alesha-pro/Qwen3.8-Flash-Next-abliterated-GSQ-RCO-Strata-GGUF` | 保留。abliterated派生を元Qwenに統合せず、対象runtime名未確認 |
| `JonathanColetti/Qwen3.8-27B-Uncensored-GGUF` | 保留。uncensored派生を元Qwenに統合せず、対象runtime名未確認 |
| `JetBrains/Mellum2.1-12B-A2.5B-Thinking-GGUF` | 採用。公式量子化配布をThinkingのLM aliasへ統合 |
| `AtomicChat/LFM2.5-24B-A2B-Uncensored-GGUF` | 保留。uncensored派生を元LFMに統合せず、対象runtime名未確認 |
| `black-forest-labs/flux-3-image-klein-pup` | 保留。画像向け派生モデルで、対象runtime名未確認 |
| `Aleph-Alpha/Kolibri-1` | 保留。独自kolibri1アーキテクチャで、対象runtime名未確認 |
| `ukisai/Swift-1.5-Qwen3.8-27B-GSQ-RCO-GGUF` | 保留。Swift派生を元Qwenに統合せず、対象runtime名未確認 |
| `microsoft/UserRepGen-4B` | 保留。ユーザー表現生成の専用派生モデルで、対象runtime名未確認 |
| `nvidia/Muse-Spark-Open-816B-A42B-DFlash2` | 保留。816B本体用の投機デコード補助重みを単独chatモデルとして扱わない |
| `nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB-LiLiCorr` | 保留。補正用派生をBolt本体と統合せず、対象runtime名未確認 |
| `nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB-LiLiCorr-GGUF` | 保留。補正用派生をBolt本体と統合せず、対象runtime名未確認 |
| `nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-BF16` | 採用。Boltの一次配布元。Super VLとは別モデル |
| `nvidia/NVIDIA-Nemotron-3.5-Super-VL-120B-A12B-NVFP4-LiLiCorr` | 保留。補正用派生をSuper VL本体と統合せず、対象runtime名未確認 |
| `nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-GGUF` | 保留。専用llama.cpp-dory配布で、Ollama/LM名未確認 |
| `nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-MXFP8` | 保留。Doryの別精度配布で、対象runtime名未確認 |
| `nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-NVFP4` | 保留。Doryの別精度配布で、対象runtime名未確認 |

## 量子化サフィックス方針（G-3、暫定実装済み）

モデル ID には量子化サフィックス（`:Q4_K_M`、`:Q5_K_M`、`:Q8_0`、`:F16`、
`:IQ4_XS` 等）が含まれるケースと含まれないケースが混在する。後方互換性のため
**ID 自体は変更せず**、`/v1/models` レスポンスに新フィールド `quantization` を
追加して情報を別出しする方針を採用する。

### 実装（`llmlb/src/models/mapping.rs`）

- `split_quantization_suffix(model_id) -> (&str, Option<&str>)`:
  - `:Q[0-9]*`、`:IQ[0-9]*`、`:F16` / `:F32` / `:BF16` / `:FP16` / `:FP32` /
    `:F8E4M3FN` / `:F8E5M2` のいずれかにマッチした場合のみ suffix を分離。
  - Ollama 形式タグ（`:30b`、`:latest` 等）は誤検出しない。
- `CanonicalResolution::canonical_for(model_key)`:
  - 通常の lookup → 量子化サフィックスを除去した base での再 lookup → self-fallback
    の順で解決。これにより `ggml-org/repo:Q4_K_M` も `BUILTIN_MAPPINGS` に登録した
    base 名（`ggml-org/repo`）から canonical を引ける。

### レスポンスフィールド

```text
{
  "id": "ggml-org/gemma-4-E4B-it-GGUF:Q4_K_M",   // 互換のため変更しない
  "quantization": "Q4_K_M",                       // 新フィールド（追加）
  ...
}
```

量子化サフィックスを持たないモデルは `quantization: null`。

### 構造的分離（後方互換性影響あり）について

「`id` を量子化なしへ正規化し、エンドポイントへのルーティング時に `quantization`
を別経路で扱う」構造的分離はクライアント・dashboard・OpenAI 互換契約への影響が
大きい。SPEC #575 US-029 では requestable な detail ID を維持し、canonical と
variant の集約情報を別に提供する互換方針を採用したため、G-3-struct の破壊的な ID
正規化案は superseded とする。

## 異常検知ログ（C-defensive）

`EndpointRegistry::sync_models()` は単一エンドポイントが
`SUSPICIOUS_MODEL_COUNT_THRESHOLD`（既定 50）件超のモデルを申告した場合に
`tracing::warn!()` を出力する。これは「カタログ集約サーバが実体を持たない全モデルを
`/v1/models` で返してしまう」誤申告（C-1 / C-2 / G-6 で観測）を運用者が早期に
気付くための観測ポイント。

## `lifecycle_status` / `download_progress` の扱い（B-4）

両フィールドは現状ダッシュボード UI（`ModelsTable.tsx`）が消費している。値は
ほぼ常に `Registered` / `null` だが、UI 連動のため API レスポンスからは削除しない。
状態管理を実装するか、フィールドそのものを廃止するかは別 SPEC で扱う。

## Issue #643 の対応状況と残課題

- G-1 / G-5 / C-3: Issue #643 で公式モデル ID、派生関係、runtime alias、context
  fallback を一次情報と照合し、`BUILTIN_MAPPINGS` と本書へ反映済み。
- G-3-struct: SPEC #575 US-029 の互換方針により superseded。requestable な `id` は
  維持し、canonical / variant 情報で集約する。
- B-4: `lifecycle_status` / `download_progress` は UI/API のプロダクト判断が必要なため、
  lifecycle 状態管理の実装またはフィールド廃止を別 owner/decision として扱う。
- C-1 / C-2 / G-6: 単一エンドポイントの大量モデル誤申告は operator / endpoint 側の
  責務として扱う。llmlb 側は `sync_models()` の警告ログで早期検知する。

## 関連リンク

- 設計概要: [`architecture.md`](./architecture.md)
- mapping 実装: `llmlb/src/models/mapping.rs`
- /v1/models 実装: `llmlb/src/api/openai.rs` (`list_models`, `get_model`)
- 親 SPEC: [#575 OpenAI互換APIゲートウェイ](https://github.com/akiojin/llmlb/issues/575)
- Follow-up Issue: [#643 /v1/models 品質改善: 残課題](https://github.com/akiojin/llmlb/issues/643)
- 鮮度検査: [#776 canonical マッピングの鮮度検査](https://github.com/akiojin/llmlb/issues/776)、
  `scripts/checks/check-mapping-freshness.sh`
