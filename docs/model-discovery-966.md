# 2026-11-14 のモデル候補確認（Issue #966）

[Issue #966](https://github.com/akiojin/llmlb/issues/966) の全27候補を、
[カタログ更新方針](model-catalog-policy.md#canonical-テーブルの更新手順issue-776)に従って確認した。
fetch 後の `HEAD` / `origin/work/issue-966` / `origin/develop` はすべて
`7aa2d863c1a9fde37166ba5df51460871932222c` で、未提供ソースと既存 PR はなかった。
全27件の `https://huggingface.co/api/models/<ID>` は HTTP 200 と ID 一致を返した。
うち16件は [Issue #958](model-discovery-958.md) または
[Issue #946](model-discovery-946.md) と重複する。今回も実在とカードを確認し、
既存対応を維持する。存在の確認だけで runtime alias は登録しない。

## 追加する対応

追加しない。Ollama または LM Studio の固定名を、このリポジトリの一次配布として
確認できた候補はなかった。対応表と context fallback は変更しない。

確認できたが採用しなかった runtime 名は次のとおり。

| 名前 | 確認結果 | 採用しない理由 |
|---|---|---|
| `huihui_ai/Qwen3.8-abliterated` の `27b-q4_K_M` ほか固定タグ。`latest` と無量子化の `27b` も含む | [Ollama tags](https://ollama.com/huihui_ai/Qwen3.8-abliterated/tags) が HTTP 200 | カードの案内はタグなしの `ollama run` だけで、同一リポジトリに Swift / Ternary / GSQ など別系列が混在する。元 Qwen へは統合しない |
| `longwayxu/xiaomi-ocr-0:bf16` | [Ollama tags](https://ollama.com/longwayxu/xiaomi-ocr-0/tags) のタグは `bf16` のみ | 前回と同じく experimental で、専用前処理に依存する |
| `qwen/qwen3.5-4b` | [LM Studio catalog](https://lmstudio.ai/models/qwen/qwen3.5-4b) が HTTP 200 | 原モデルの catalog であり、GLB-3D-Coder の fine-tune ではない |

`NousResearch/Arete-27B`、`webAI-Official/TwIL-8B`、`maritaca-ai/S5-nano`、
`Axion1-0/Axion-1.0-35B`、`Nanbeige/Nanbeige4.3-33B`、`taobao-mnn/MNN-Pocket-0.3B`、
`nvidia` の Dory について、想定した Ollama library と LM Studio catalog の URL は
404 だった。Ollama 検索も当該モデルのライブラリ項目を返さなかった。
`orcarouter` の2件は Models API が 200 で、README 本文の取得は 401 だった。
判定は API の card を使う。

## 全候補の判定

各リンクは一次配布元を指す。保留は今回の範囲で runtime 名や運用条件の確認が不足した
ことを意味し、モデルや community 配布が存在しないという判断ではない。
`base_model` の指定だけでは fine-tune / merge / abliteration を同一 canonical にしない。
判定は採用 0 件、保留 26 件、不採用 1 件。

| HF リポジトリ ID | 判定・根拠 |
|---|---|
| [xtools-at/DeepSeek-V4-Pro-Qwen3.5-9B-UNCENSORED-heretic](https://huggingface.co/xtools-at/DeepSeek-V4-Pro-Qwen3.5-9B-UNCENSORED-heretic) | 保留。Heretic による decensor 済み fine-tune。対象 runtime 名は未確認 |
| [DavidAU/Qwen3.8-27B-TURBO-Fable-Cold-Fusion-735-882-Heretic-Uncensored-NEO-CODER-MAX-MTP-GGUF](https://huggingface.co/DavidAU/Qwen3.8-27B-TURBO-Fable-Cold-Fusion-735-882-Heretic-Uncensored-NEO-CODER-MAX-MTP-GGUF) | 保留を維持。複数系統の merge と Heretic。元 Qwen へ統合しない |
| [NousResearch/Arete-27B](https://huggingface.co/NousResearch/Arete-27B) | 保留。Qwen3.8-27B の SFT。公式手順に Ollama / LM Studio 名はなく、元 Qwen へ統合しない |
| [webAI-Official/TwIL-8B](https://huggingface.co/webAI-Official/TwIL-8B) | 保留。同一リポジトリの GGUF と llama.cpp コマンドはある。Ollama / LM Studio の固定名は未確認 |
| [ccore/Qwen3.5-4B-GLB-3D-Coder](https://huggingface.co/ccore/Qwen3.5-4B-GLB-3D-Coder) | 保留。Qwen3.5-4B の fine-tune。LM Studio への言及は再ダウンロード手順で、catalog 名ではない |
| [RikiyaT/Qwen3.5-0.8B-weight-space-programming](https://huggingface.co/RikiyaT/Qwen3.5-0.8B-weight-space-programming) | 保留。Qwen3.5-0.8B の fine-tune。案内は vLLM のみ |
| [tencent/Qwen3.8-27B-DSpine-b16](https://huggingface.co/tencent/Qwen3.8-27B-DSpine-b16) | 保留。Qwen3.8-27B 用の draft。単独の会話モデルとして扱わず、元モデルへ統合しない |
| [DavidAU/Qwen3.5-9B-The-Defiant-Fable-Uncensored-Heretic-NEO-IMATRIX-MAX-MTP-GGUF](https://huggingface.co/DavidAU/Qwen3.5-9B-The-Defiant-Fable-Uncensored-Heretic-NEO-IMATRIX-MAX-MTP-GGUF) | 保留を維持。複数 fine-tune の merge。元 Qwen へ統合しない |
| [sensenova/SenseNova-Loop-9B-Preview](https://huggingface.co/sensenova/SenseNova-Loop-9B-Preview) | 保留を維持。独自 loop 構成で、公式手順は未リリースの vLLM / SGLang。対象 runtime 名は未確認 |
| [huihui-ai/Huihui-Qwen3.8-27B-abliterated-GGUF](https://huggingface.co/huihui-ai/Huihui-Qwen3.8-27B-abliterated-GGUF) | 保留を維持。Ollama の固定タグは実在するが、系列が混在し元 Qwen へ統合しない |
| [baidu/Unlimited-OCR-2](https://huggingface.co/baidu/Unlimited-OCR-2) | 保留を維持。公式手順は Transformers と専用 SGLang wheel。対象 runtime 名は未確認 |
| [HauhauCS/Qwen3.8-27B-Uncensored-HauhauCS-Aggressive-MTP-GGUF](https://huggingface.co/HauhauCS/Qwen3.8-27B-Uncensored-HauhauCS-Aggressive-MTP-GGUF) | 保留を維持。LM Studio で読める旨の一般案内はあるが、Aggressive 派生の固定名は未確認 |
| [AtomicChat/LFM3-1.7B-Uncensored-GGUF](https://huggingface.co/AtomicChat/LFM3-1.7B-Uncensored-GGUF) | 保留を維持。refusal を変更した派生。案内は llama.cpp のみで、元 LFM3 へ統合しない |
| [xiaomi-research/Xiaomi-OCR-0](https://huggingface.co/xiaomi-research/Xiaomi-OCR-0) | 保留を維持。community の `bf16` タグは残っているが experimental のまま |
| [prism-ml/Ternary-Bonsai-2-27B-gguf](https://huggingface.co/prism-ml/Ternary-Bonsai-2-27B-gguf) | 保留を維持。独自 ternary kernel。対象 runtime 名は未確認 |
| [Nanbeige/Nanbeige4.3-33B](https://huggingface.co/Nanbeige/Nanbeige4.3-33B) | 保留を維持。公式手順は Transformers / SGLang / vLLM。対象 runtime 名は未確認 |
| [microsoft/Mage-VL-Cowork](https://huggingface.co/microsoft/Mage-VL-Cowork) | 保留を維持。custom `llava_onevision2` と専用 SGLang。標準 Qwen へ統合しない |
| [DavidAU/Qwen3.8-27B-Ultimate-Heretic-Uncensored-Turbo-Brilliance-50x-AGI-NEO-GGUF](https://huggingface.co/DavidAU/Qwen3.8-27B-Ultimate-Heretic-Uncensored-Turbo-Brilliance-50x-AGI-NEO-GGUF) | 保留。Heretic 済みの独自派生。対象 runtime 名は未確認 |
| [ML-Intern-lab/Qwen3.5-0.8B-weight-space-programming](https://huggingface.co/ML-Intern-lab/Qwen3.5-0.8B-weight-space-programming) | 保留。Qwen3.5-0.8B の fine-tune。対象 runtime 名は未確認 |
| [Axion1-0/Axion-1.0-35B](https://huggingface.co/Axion1-0/Axion-1.0-35B) | 保留。再現チェックポイント。公式手順は vLLM の `--served-model-name axion-35b` のみ |
| [taobao-mnn/MNN-Pocket-0.3B](https://huggingface.co/taobao-mnn/MNN-Pocket-0.3B) | 保留。主 runtime は MNN。公式 GGUF は llama.cpp 向けで、Ollama / LM Studio 名は未確認 |
| [orcarouter/OrcaCyber-Nano-1.0](https://huggingface.co/orcarouter/OrcaCyber-Nano-1.0) | 保留を維持。`base_model` は Qwen だが独自配布。README は 401 で、対象 runtime 名は未確認 |
| [maritaca-ai/S5-nano](https://huggingface.co/maritaca-ai/S5-nano) | 保留。BF16 は vLLM / SGLang。姉妹 GGUF は llama.cpp 向けで、Ollama / LM Studio 名は未確認 |
| [JonathanColetti/Qwen3.8-27B-Uncensored-GGUF](https://huggingface.co/JonathanColetti/Qwen3.8-27B-Uncensored-GGUF) | 保留を維持。Heretic と abliteration で重みを変更。quantized メタデータだけでは元 Qwen へ統合しない |
| [nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-BF16](https://huggingface.co/nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-BF16) | 保留を維持。公式手順は専用 llama.cpp-dory と未リリースの vLLM。対象 runtime 名は未確認 |
| [LiquidAI/d1-2B](https://huggingface.co/LiquidAI/d1-2B) | 不採用を維持。生成トークンを返さない意思決定モデル。通常生成モデルの対応表には追加しない |
| [orcarouter/Flint-abliterated-GGUF](https://huggingface.co/orcarouter/Flint-abliterated-GGUF) | 保留。`RecursiveSI/flint` の abliteration。README は 401。既存 flint へ統合しない |

この確認は名前対応の根拠であり、各モデルの重みを用いたローカル推論試験ではない。
未確認の xLLM / vLLM alias、可変 `latest`、量子化ファイル名は追加しない。

## 検証範囲

`mapping::tests::test_issue_966_candidates_are_not_merged_into_existing_models` で、
上表の ID と採用しなかった runtime 名が未解決のままであること、および既存の
Qwen3.8-27B、LFM3-1.7B、flint がそれぞれの確認済み別名へ解決し続けることを検証する。
正式な結果は PR の fresh `verify.run` 記録に残す。

User Verification Result: n/a (autonomous)

Agent Visual Check: n/a (no UI surface)
