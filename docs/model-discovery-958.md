# 2026-11-02 のモデル候補確認（Issue #958）

[Issue #958](https://github.com/akiojin/llmlb/issues/958) の全29候補を、
[カタログ更新方針](model-catalog-policy.md#canonical-テーブルの更新手順issue-776)に従って確認した。
fetch 後の `HEAD` / `origin/work/issue-958` / `origin/develop` はすべて
`f684b815d739f101d90470b60e34db7b0784cace` で、未提供ソースと既存 PR はなかった。
全29件の `https://huggingface.co/api/models/<ID>` は HTTP 200 を返した。
うち10件は[前回の確認](model-discovery-946.md)と重複する。今回も実在とカードを確認し、
既存対応を維持する。存在の確認だけで runtime alias は登録しない。

## 追加する対応

3 canonical を追加する。候補として数えると公式 GGUF 2件を含む5件に対応する。
既存原モデルで対応済みが2件、保留が20件、不採用が2件で、計29件となる。

| canonical | Ollama 固定名（先頭が優先名） | LM Studio 名（先頭が優先名） | context fallback |
|---|---|---|---:|
| `LiquidAI/LFM3-1.7B` | `lfm3:1.7b`, `lfm3:1.7b-q4_K_M`, `lfm3:1.7b-q8_0`, `lfm3:1.7b-bf16` | `LiquidAI/LFM3-1.7B-GGUF` | 262144 |
| `LiquidAI/LFM3-VL-2B` | `lfm3-vl:2b`, `lfm3-vl:2b-q4_K_M`, `lfm3-vl:2b-q8_0`, `lfm3-vl:2b-bf16` | `liquid/lfm3-vl-2b`, `lmstudio-community/LFM3-VL-2B-GGUF`, `LiquidAI/LFM3-VL-2B-GGUF` | 128000 |
| `XHToken/Spark-X2.6-1.7B` | `SparkLLM/Spark-X2.6-1.7B:q4_k_m`, `SparkLLM/Spark-X2.6-1.7B:q8_0` | `XHToken/Spark-X2.6-1.7B-GGUF` | 1048576 |

### 一次情報

- LFM3 text: [公式 GGUF](https://huggingface.co/LiquidAI/LFM3-1.7B-GGUF) の
  `base_model` は `LiquidAI/LFM3-1.7B`。README は LM Studio の利用と
  llama.cpp engine 2.65.1 以降を案内する。
  [Ollama tags](https://ollama.com/library/lfm3/tags)で上表の固定タグを確認した。
  [公式 config](https://huggingface.co/LiquidAI/LFM3-1.7B/blob/main/config.json)の
  `max_position_embeddings` は262144。
  `liquid/lfm3-1.7b` の LM Studio catalog は404だったため、その名前は追加しない。
- LFM3 VL: [公式 GGUF](https://huggingface.co/LiquidAI/LFM3-VL-2B-GGUF)の
  `base_model` は `LiquidAI/LFM3-VL-2B`。
  [LM Studio catalog](https://lmstudio.ai/models/liquid/lfm3-vl-2b)と、そこから参照される
  [community GGUF](https://huggingface.co/lmstudio-community/LFM3-VL-2B-GGUF)の原モデルを確認した。
  [Ollama tags](https://ollama.com/library/lfm3-vl/tags)で上表の固定タグを確認した。
  [公式 config](https://huggingface.co/LiquidAI/LFM3-VL-2B/blob/main/config.json)の
  `text_config.max_position_embeddings` は**128000**であり、131072とはしない。
  公式 GGUF は LM Studio engine 2.65.1 以降、Ollama v0.40.12 以降を案内する。
  catalog の画像対応から動画対応を推測しない。
- Spark: [公式 GGUF](https://huggingface.co/XHToken/Spark-X2.6-1.7B-GGUF)の
  `base_model` は `XHToken/Spark-X2.6-1.7B`。README は Ollama 0.34.1 以降と
  LM Studio 0.4.0 以降の利用を案内する。
  [公式 Ollama tags](https://ollama.com/SparkLLM/Spark-X2.6-1.7B/tags)で固定タグを確認した。
  [公式 config](https://huggingface.co/XHToken/Spark-X2.6-1.7B/blob/main/config.json)の
  `max_position_embeddings` は1048576。既存4Bモデルと別 SKU にする。

context は欠損時だけ補完し、エンドポイントの申告を優先する。
この確認は名前対応の根拠であり、各モデルの重みを用いたローカル推論試験ではない。
未確認の xLLM / vLLM alias、可変 `latest`、量子化ファイル名は追加しない。

## 全候補の判定

各リンクは一次配布元を指す。保留は今回の範囲で runtime 名や運用条件の確認が不足した
ことを意味し、モデルや community 配布が存在しないという判断ではない。
`base_model` の指定だけでは fine-tune / merge / abliteration を同一 canonical にしない。

| HF リポジトリ ID | 判定・根拠 |
|---|---|
| [LiquidAI/LFM3-1.7B](https://huggingface.co/LiquidAI/LFM3-1.7B) | 採用。公式固定タグと公式 GGUF の LM Studio 案内を確認 |
| [baidu/Unlimited-OCR-2](https://huggingface.co/baidu/Unlimited-OCR-2) | 保留。公式手順は Transformers と専用 SGLang wheel。対象 runtime 名は未確認 |
| [Nanbeige/Nanbeige4.3-33B](https://huggingface.co/Nanbeige/Nanbeige4.3-33B) | 保留を維持。公式手順は Transformers / SGLang / vLLM、対象 runtime 名は未確認 |
| [Diffuspace/Clover-35B-A3B-Preview](https://huggingface.co/Diffuspace/Clover-35B-A3B-Preview) | 保留。独自 block diffusion と公式 SGLang / llama.cpp 実装。元の Qwen には統合しない |
| [LiquidAI/LFM3-VL-2B](https://huggingface.co/LiquidAI/LFM3-VL-2B) | 採用。公式固定タグ、LM Studio catalog と GGUF を確認 |
| [AtomicChat/LFM3-1.7B-Uncensored-GGUF](https://huggingface.co/AtomicChat/LFM3-1.7B-Uncensored-GGUF) | 保留。refusal を変更した独自派生。元 LFM3 に統合せず、対象 runtime 名も未確認 |
| [LiquidAI/LFM3-1.7B-GGUF](https://huggingface.co/LiquidAI/LFM3-1.7B-GGUF) | 採用。公式量子化配布を原モデルの LM Studio alias に追加 |
| [LiquidAI/d1-2B](https://huggingface.co/LiquidAI/d1-2B) | 不採用。生成トークンを返さない意思決定モデル。通常生成モデルの対応表には追加しない |
| [xiaomi-research/Xiaomi-OCR-0](https://huggingface.co/xiaomi-research/Xiaomi-OCR-0) | 保留。公式が案内する community Ollama BF16 タグは実在するが、experimental で専用前処理・prompt / MCP・Demo に依存。一般の生成モデルとしての対応確認は不足 |
| [sensenova/SenseNova-Loop-9B-Preview](https://huggingface.co/sensenova/SenseNova-Loop-9B-Preview) | 保留。独自 looped multimodal 構成。対象 runtime 名は未確認 |
| [nvidia/NVIDIA-Nemotron-3.5-Bolt-E4B-4GB](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-E4B-4GB) | 既存原モデル対応。前回の E4B BF16 canonical を維持し、配布別 context を推測しない |
| [DavidAU/Qwen3.8-27B-TURBO-Fable-Cold-Fusion-735-882-Heretic-Uncensored-NEO-CODER-MAX-MTP-GGUF](https://huggingface.co/DavidAU/Qwen3.8-27B-TURBO-Fable-Cold-Fusion-735-882-Heretic-Uncensored-NEO-CODER-MAX-MTP-GGUF) | 保留を維持。独自派生で原配布・runtime の対応を未確定 |
| [microsoft/Mage-VL-Cowork](https://huggingface.co/microsoft/Mage-VL-Cowork) | 保留。custom llava_onevision2 と専用 SGLang コンテナ / generate 手順。標準 Qwen へ統合しない |
| [divagr1925/Qwen3.5-4B-Qwenputer](https://huggingface.co/divagr1925/Qwen3.5-4B-Qwenputer) | 保留。base_model_relation は merge。元 Qwen へ統合せず、対象 runtime 名は未確認 |
| [LiquidAI/LFM3-1.7B-Base](https://huggingface.co/LiquidAI/LFM3-1.7B-Base) | 保留。事前学習版は post-trained 版と別 SKU。対応する固定 runtime 名は未確認 |
| [huihui-ai/Huihui-Qwen3.8-27B-abliterated-GGUF](https://huggingface.co/huihui-ai/Huihui-Qwen3.8-27B-abliterated-GGUF) | 保留を維持。abliteration で変更された独自派生を元 Qwen へ統合しない |
| [openbmb/MiniCPM-OCR-1.0](https://huggingface.co/openbmb/MiniCPM-OCR-1.0) | 保留。公式が Ollama 対応を案内するが、SDK の layout / parsing pipeline と固定 runtime 名の確認が必要 |
| [openbmb/MiniCPM-OCR-Jev](https://huggingface.co/openbmb/MiniCPM-OCR-Jev) | 不採用。LM head を持たず2画像を評価する意思決定モデル。通常の生成 endpoint は利用不可 |
| [sensenova/SenseNova-6.8-Code2Video](https://huggingface.co/sensenova/SenseNova-6.8-Code2Video) | 保留。Qwen fine-tune、公式手順は vLLM / SGLang。対象 runtime 名は未確認 |
| [JonathanColetti/Qwen3.8-27B-Uncensored-GGUF](https://huggingface.co/JonathanColetti/Qwen3.8-27B-Uncensored-GGUF) | 保留を維持。カード本文の重み変更を優先し、quantized メタデータだけで元 Qwen へ統合しない |
| [HauhauCS/Qwen3.8-27B-Uncensored-HauhauCS-Aggressive-MTP-GGUF](https://huggingface.co/HauhauCS/Qwen3.8-27B-Uncensored-HauhauCS-Aggressive-MTP-GGUF) | 保留を維持。LM Studio 案内はあるが独自 Aggressive 派生。原配布との identity が未確定 |
| [HuggingFaceBio/Carbon-X](https://huggingface.co/HuggingFaceBio/Carbon-X) | 保留を維持。Qwen から追加学習した別モデル。公式 GGUF の存在だけで対象 runtime alias を補完しない |
| [XHToken/Spark-X2.6-1.7B](https://huggingface.co/XHToken/Spark-X2.6-1.7B) | 採用。公式固定タグと公式 GGUF の runtime 案内を確認 |
| [InternScience/Sage-A1.5](https://huggingface.co/InternScience/Sage-A1.5) | 保留を維持。公式手順は vLLM / SGLang 等。対象 runtime 名は未確認 |
| [DavidAU/Qwen3.5-9B-The-Defiant-Fable-Uncensored-Heretic-NEO-IMATRIX-MAX-MTP-GGUF](https://huggingface.co/DavidAU/Qwen3.5-9B-The-Defiant-Fable-Uncensored-Heretic-NEO-IMATRIX-MAX-MTP-GGUF) | 保留を維持。複数 fine-tune の merge。元 Qwen への統合や runtime 名の推測はしない |
| [unsloth/flint-GGUF](https://huggingface.co/unsloth/flint-GGUF) | 既存原モデル対応。前回の RecursiveSI/flint canonical と catalog 優先名を維持 |
| [huihui-ai/Huihui-DeepSeek-V4.1-Flash-1017-abliterated-GGUF](https://huggingface.co/huihui-ai/Huihui-DeepSeek-V4.1-Flash-1017-abliterated-GGUF) | 保留。abliteration を適用した独自派生。元 DeepSeek に統合せず、Maya / Strata 向け配布も対象名とはみなさない |
| [LiquidAI/LFM3-VL-2B-GGUF](https://huggingface.co/LiquidAI/LFM3-VL-2B-GGUF) | 採用。公式量子化配布を原モデルの LM Studio alias に追加 |
| [kaitchup/Qwen3.8-27B-SLQ-GGUF](https://huggingface.co/kaitchup/Qwen3.8-27B-SLQ-GGUF) | 保留。量子化元は既存 Qwen だが、実測は llama.cpp の text scoring。対象 runtime 名は未確認 |

Xiaomi は[公式カード](https://huggingface.co/xiaomi-research/Xiaomi-OCR-0)と
[Ollama tags](https://ollama.com/longwayxu/xiaomi-ocr-0/tags)を確認した。
Jev の非生成モデルという判断は[公式 llama.cpp 手順](https://github.com/OpenBMB/MiniCPM-OCR/blob/main/docs/llamacpp_deploy_jev_en.md)
の LM head・text-generation endpoint の制約にも基づく。
モデル検知アルゴリズムや新しい推論プロトコルへの対応は本作業に含めない。

## 検証範囲

`mapping::tests::test_issue_958_*` で双方向解決、優先名、engine の境界、大小文字、
正確な context fallback、エンドポイント値の優先、base / 独自派生 / 意思決定モデルの分離、
既存 Spark 4B の維持を検証する。既存 mapping テスト、discovery のオフラインテスト、
`make quality-checks` も実行し、正式な結果は PR の fresh `verify.run` 記録に残す。

User Verification Result: n/a (autonomous)

Agent Visual Check: n/a (no UI surface)
