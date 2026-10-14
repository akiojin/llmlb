# Project Maya の source-derived fixtures の出典

これらの JSON は最新安定 release v1.1.8 の固定 source から導出した例です。
HTTP 応答の実測ではありません。manifest の provenance.kind は source-derived です。
起動した model、実測した engine version、実測した HTTP status を表しません。

## 出典

- Upstream: <https://github.com/mw00/project-maya>
- Release: <https://github.com/mw00/project-maya/releases/tag/v1.1.8>
- 公開日時: 2026-10-14T04:38:22Z
- Commit: 5c970a863ce2fdd87b6e23ffb4822f6d76522b1c
- Commit 日時: 2026-10-14T04:25:22Z

`source-snippets.txt` に固定 commit permalink と行番号付きの実 source を保存しています。
`selected-settings.json` に選択設定・前提・値の選択理由を保存しています。
この `PROVENANCE.md` は fixture 専用の証跡説明です。

[health 組立][health]、[models 組立][models]、[props 組立][props]、[認証][auth]を参照してください。
JSON の Content-Type は [_json][json] に明記された値のみを使います。
Date、Server、raw HTTP response は作成していません。

## 選択した値

| フィールド | 選択値 | 出典・理由 |
| --- | --- | --- |
| health.version | 1.1.8 | VERSION の固定値を server.py:58 が読む |
| health.status | ok | update/reload のない平常状態を前提に選択 |
| health.max_context | 4096 | 説明用の設定値。実測値ではない |
| health.model | glm-5.3-flash | maya.py:90,1819 の公式 model_name |
| health.images | false | vision 未設定の例 |
| health.api_key | false / true | bool(svc.api_key)。キーなし/設定ありの例 |
| health.defaults.temperature | 1.0 | maya.py:91 の installer 既定値 |
| health.defaults.top_p | 0.95 | maya.py:91 の installer 既定値 |
| health.defaults.reasoning_effort | high | maya.py:92 の installer 既定値 |
| health.mcp_tools | false / true | bool(svc.mcp)。Hub なし/構成ありの例 |
| models.data | 1 model | engine alive を前提に選択。観測値ではない |
| models.data[0].status.value | loaded | handler の固定文字列 |
| models.data[0].meta.n_ctx | 4096 | 選択した engine max_context と一致 |
| models.data[0].architecture | text 入出力 | vision 未設定の例 |
| props.total_slots | 1 | multi=0 を前提とした max(1, multi) |
| props.default_generation_settings | n_ctx=4096 | 選択した max_context と一致 |
| props.params.n_predict | -1 | shared.max_tokens 未設定の handler 既定値 |
| props.model_alias | glm-5.3-flash | 選択した model_name |
| props.model_path | pack/full | 選択 args から server.py:411-412 が導出する既定値 |
| props.chat_template | upstream 全文 | tokenizer 側に template がない場合の fallback source |
| props.modalities.vision | false | vision 未設定の例 |
| props.models_autoload | false | handler の固定値 |
| props.is_sleeping | false | handler の固定値 |

`props-deterministic-fields.json` は選択状態から導出できる dict の projection です。
[StrataEngine の model_path 組立][model-path]は `--native` または `--pack` のみを検索します。
選択 args には `--glm-pack` があり、検索対象の option はないため paths は空です。
その結果 model_path は既定の `pack/full` となり、[props への追加条件][props-path]を満たします。
この値は固定 source と選択設定から導出できるため fixture に含めています。
実際にロードしたモデルの場所を観測した値ではありません。
未観測の engine.info.version による build_info のみを省略しています。
この範囲を manifest の body_scope / omitted_optional_fields に明記しています。

MCP ありの例は `mcp_servers` に1件の HTTP server 設定を選択します。
設定例 URL は説明用であり、存在・接続成功を主張しません。
[hub_from_config][mcp] は有効な server 設定がなければ None、あれば McpHub を返します。
そのため mcp_tools は tool 数や接続成功ではなく、Hub 構成の有無を表します。

## 認証条件

manifest.responses は以下の13件です。

- auth-disabled-no-bearer: 3 path とも source-derived status 200。
- auth-enabled-no-bearer: health は200、models/props は401。
- auth-enabled-valid-bearer: 3 path とも200。
- auth-enabled-invalid-bearer: health は200、models/props は401。
- mcp-enabled: api_key=false, mcp_tools=true の health 例、status 200。

api_key=true, mcp_tools=true の health 例も別 JSON と additional_examples に保存しています。
したがって api_key と mcp_tools の4通りを確認できます。
`/health` 自体は `_authorized()` を呼ばず、認証有無にかかわらず public です。
`/v1/models` と `/props` は `_authorized()` を呼びます。

models の空配列、props の404/503、update/reload 中の health は今回の fixture 対象外です。
今回の status 200 は選択した正常状態の source 条件分岐による値です。

## 実起動の証跡と限界

macOS arm64 で正規 maya.sh を起動すると platform check で exit 1 です。
追加で既存 Docker image を使い、Linux x86_64 / Debian 12 でも正規 launcher を実行しました。
image の不足していた python3.11-venv は、削除される一時 container の中だけに導入しました。
私有 .venv 作成成功後、auth 無効/有効の双方が以下の理由で exit 1 になりました。

```text
no NVIDIA GPU found (nvidia-smi did not answer)
```

HIP --check も no supported AMD GPU found で exit 1 です。
GPU tools/device nodes は存在せず、model download、engine 起動、HTTP listener 開始には至りません。
Docker container は --rm で終了済みです。稼働中 PID/URL はありません。
元 source は archive と byte 一致します。mock/fake engine は使っていません。

Docker command、platform、stdout/stderr、各 command の exit code は親 evidence の
`docker-linux/official-linux-with-venv-startup.stdout.json` と
`docker-linux/docker-with-venv-command.json` に保存しています。

`fixture-validation.json` は固定 source の辞書組立式と選択値の整合検証です。
アプリ・HTTP service・engine を生成せず、式だけを説明用の値で評価しています。
この検証は HTTP または実 engine の動作確認ではありません。

[health]: https://github.com/mw00/project-maya/blob/5c970a863ce2fdd87b6e23ffb4822f6d76522b1c/serve/server.py#L2798-L2807
[models]: https://github.com/mw00/project-maya/blob/5c970a863ce2fdd87b6e23ffb4822f6d76522b1c/serve/server.py#L2838-L2845
[props]: https://github.com/mw00/project-maya/blob/5c970a863ce2fdd87b6e23ffb4822f6d76522b1c/serve/server.py#L2923-L2946
[auth]: https://github.com/mw00/project-maya/blob/5c970a863ce2fdd87b6e23ffb4822f6d76522b1c/serve/server.py#L2723-L2731
[json]: https://github.com/mw00/project-maya/blob/5c970a863ce2fdd87b6e23ffb4822f6d76522b1c/serve/server.py#L2684-L2690
[mcp]: https://github.com/mw00/project-maya/blob/5c970a863ce2fdd87b6e23ffb4822f6d76522b1c/serve/mcp.py#L641-L659

[model-path]: https://github.com/mw00/project-maya/blob/5c970a863ce2fdd87b6e23ffb4822f6d76522b1c/serve/server.py#L411-L412
[props-path]: https://github.com/mw00/project-maya/blob/5c970a863ce2fdd87b6e23ffb4822f6d76522b1c/serve/server.py#L2941-L2945
