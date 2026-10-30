# 2026-10-30 のモデル候補確認（Issue #946）

[Issue #946](https://github.com/akiojin/llmlb/issues/946) の41候補を、
[カタログ更新方針](model-catalog-policy.md#canonical-テーブルの更新手順issue-776)に従って評価した。
fetch 後の `HEAD` / `origin/work/issue-946` / `origin/develop` はすべて
`5ee8623ded71eae4bf3d0c656357d4112eeb1e96` で、未配信ソースと既存 PR はなかった。
41件すべての `https://huggingface.co/api/models/<ID>` が HTTP 200 を返した。
存在することと、llmlb の対象 runtime で確認できたことは区別する。

## 採用する対応

4 canonical と、既存 Qwen canonical の量子化 alias を追加する。
推論処理や名前解決規則は既存のままとし、未確認の runtime 名は補完しない。

| canonical | Ollama の固定名（先頭が優先名） | LM Studio の確認済み名（先頭が優先名） | context fallback |
|---|---|---|---|
| `RecursiveSI/flint` | `flint:40b-a2b-q4_1`, `flint:40b-a2b`, `flint:40b` | `recursivesi/flint`, `lmstudio-community/flint-GGUF` | 262144 |
| `XHToken/Spark-X2.6-4B` | `SparkLLM/Spark-X2.6-4B:q4_k_m`, `SparkLLM/Spark-X2.6-4B:q8_0` | `XHToken/Spark-X2.6-4B-GGUF` | 1048576 |
| `nvidia/NVIDIA-Nemotron-3.5-Bolt-E4B-BF16` | `nemotron-3.5-bolt:e4b` | `nvidia/nemotron-3.5-bolt-e4b`, `lmstudio-community/NVIDIA-Nemotron-3.5-Bolt-E4B-GGUF` | 追加しない（配布ごとに異なる） |
| `meta-llama/Meta-Llama-3-8B-Instruct` | `llama3:8b-instruct-q4_K_M` | 未確認のため追加しない | 8192 |
| `Qwen/Qwen3.8-27B`（既存） | 追加しない | `ISTA-DASLab/Qwen3.8-27B-GSQ-RCO-GGUF` を末尾に追加 | 既存値を維持 |

### 一次情報と境界

- Flint: [Ollama tags](https://ollama.com/library/flint/tags)、
  [LM Studio catalog](https://lmstudio.ai/models/recursivesi/flint)、
  [公式 config](https://huggingface.co/RecursiveSI/flint/blob/main/config.json)。
  catalog が指す [GGUF](https://huggingface.co/lmstudio-community/flint-GGUF) の
  `base_model` は `RecursiveSI/flint`。unsloth 版も同じ原モデルだが、その runtime 名は登録しない。
- Spark: [公式 Ollama tags](https://ollama.com/SparkLLM/Spark-X2.6-4B/tags)、
  [公式 GGUF README](https://huggingface.co/XHToken/Spark-X2.6-4B-GGUF)、
  [公式 config](https://huggingface.co/XHToken/Spark-X2.6-4B/blob/main/config.json)。
  GGUF README が Ollama / LM Studio 対応を案内し、`base_model` は上表の post-trained モデル。
  配布元が案内するバージョンは Ollama 0.34.1 以降、LM Studio 0.4.0 以降。
  `Spark-X2.6-4B-Base` は別 SKU とする。
- Bolt E4B: [公式 BF16](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-E4B-BF16)、
  [Ollama tags](https://ollama.com/library/nemotron-3.5-bolt/tags)、
  [LM Studio catalog](https://lmstudio.ai/models/nvidia/nemotron-3.5-bolt-e4b)。
  [4GB 版](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-E4B-4GB)は公式量子化版で64K、
  [unsloth 版](https://huggingface.co/unsloth/NVIDIA-Nemotron-3.5-Bolt-E4B-GGUF)と
  [catalog の GGUF](https://huggingface.co/lmstudio-community/NVIDIA-Nemotron-3.5-Bolt-E4B-GGUF)は128K。
  共通値を推測せず、エンドポイント申告を使う。Gemma E4B は学習元であり、同一モデルにはしない。
- Llama 3: [Meta のモデルカード](https://huggingface.co/meta-llama/Meta-Llama-3-8B-Instruct)、
  [Ollama tags](https://ollama.com/library/llama3/tags)。8B Instruct の固定タグだけを追加し、
  base / `8b-text` や既存 Llama 3.1 に統合しない。
- Qwen GSQ-RCO: [配布元 README](https://huggingface.co/ISTA-DASLab/Qwen3.8-27B-GSQ-RCO-GGUF) が
  LM Studio で repo 名を検索する手順を明示。重みの量子化であることと原モデルを確認した。
  `qwen/qwen3.8-27b` を優先名として維持し、ファイルごとの MTP 指定や Ollama の可変タグは追加しない。

## 全候補の判定

各 ID のリンクは一次配布元のモデルカードを指す。「原モデル採用」は、量子化配布の関係を
確認したことを示す。そのリポジトリ名を runtime alias として登録したことは意味しない。
保留は今回の確認範囲での判断であり、モデルや community 配布の不存在を意味しない。
`base_model` だけでは fine-tune / abliteration / draft を同一モデルに統合しない。

| HF リポジトリ ID | 判定・根拠 |
|---|---|
| [RecursiveSI/flint](https://huggingface.co/RecursiveSI/flint) | 採用。固定タグと LM Studio catalog を確認 |
| [XHToken/Spark-X2.6-4B](https://huggingface.co/XHToken/Spark-X2.6-4B) | 採用。公式固定タグと GGUF の runtime 案内を確認 |
| [Nanbeige/Nanbeige4.3-33B](https://huggingface.co/Nanbeige/Nanbeige4.3-33B) | 保留。community の home-rag 配布は存在するが、公式 runtime 名は未確認 |
| [InternScience/Sage-A1.5](https://huggingface.co/InternScience/Sage-A1.5) | 保留。vLLM / SGLang 中心で対象 runtime 名は未確認 |
| [unsloth/flint-GGUF](https://huggingface.co/unsloth/flint-GGUF) | 原モデル採用。Flint への関係を確認、alias は catalog の配布元を優先 |
| [HuggingFaceBio/Carbon-X](https://huggingface.co/HuggingFaceBio/Carbon-X) | 保留。公式 GGUF は存在するが対象 runtime 名は未確認 |
| [DavidAU/Qwen3.8-27B-TURBO-Fable-Cold-Fusion-735-882-Heretic-Uncensored-NEO-CODER-MAX-MTP-GGUF](https://huggingface.co/DavidAU/Qwen3.8-27B-TURBO-Fable-Cold-Fusion-735-882-Heretic-Uncensored-NEO-CODER-MAX-MTP-GGUF) | 保留。派生モデルで、原配布・runtime の対応を確定できず |
| [KRAFTON/Raon-1-mini-preview](https://huggingface.co/KRAFTON/Raon-1-mini-preview) | 保留。公式手順が専用 fork を要求し、upstream support は pending |
| [jialinyyzz/humanizer](https://huggingface.co/jialinyyzz/humanizer) | 保留。公式 USAGE は Ollama / LM Studio 未実機検証と明記。専用 template が必要 |
| [orcarouter/OrcaCyber-Nano-1.0](https://huggingface.co/orcarouter/OrcaCyber-Nano-1.0) | 保留。DFlash 補助重みを含む派生配布で runtime 名未確定 |
| [nvidia/NVIDIA-Nemotron-3.5-Bolt-E4B-4GB](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-E4B-4GB) | 原モデル採用。公式 quantized 関係から E4B BF16 に対応 |
| [XHToken/Spark-X2.6-4B-GGUF](https://huggingface.co/XHToken/Spark-X2.6-4B-GGUF) | 採用。公式 LM Studio 配布名として登録 |
| [HauhauCS/Qwen3.8-27B-Uncensored-HauhauCS-Aggressive-MTP-GGUF](https://huggingface.co/HauhauCS/Qwen3.8-27B-Uncensored-HauhauCS-Aggressive-MTP-GGUF) | 保留。LM Studio 案内はあるが、uncensored 派生の identity 確認を完了していない。元 Qwen には統合しない |
| [ttindall/Qwen3.8-27B-Defiance-Uncensored-MTP-GGUF](https://huggingface.co/ttindall/Qwen3.8-27B-Defiance-Uncensored-MTP-GGUF) | 保留。複数派生の重みと template 変更があり、canonical の対応を未確定 |
| [MemTensor/MemT-1.0-Nano](https://huggingface.co/MemTensor/MemT-1.0-Nano) | 保留。Qwen の fine-tune で対象 runtime 名は未確認 |
| [unsloth/NVIDIA-Nemotron-3.5-Bolt-E4B-GGUF](https://huggingface.co/unsloth/NVIDIA-Nemotron-3.5-Bolt-E4B-GGUF) | 原モデル採用。E4B BF16 の量子化。runtime alias は公式 catalog を優先 |
| [tencent/Hy-Memory-30B-A3B](https://huggingface.co/tencent/Hy-Memory-30B-A3B) | 保留。独自 hy_v3 アーキテクチャで対象 runtime 名は未確認 |
| [nvidia/Qwen3.8-Flash-Next-FP4-Local](https://huggingface.co/nvidia/Qwen3.8-Flash-Next-FP4-Local) | 保留。Model Optimizer 配布で対象 runtime 名未確認。圧縮後パラメータ数を原本規模とみなさない |
| [ISTA-DASLab/Qwen3.8-27B-GSQ-RCO-GGUF](https://huggingface.co/ISTA-DASLab/Qwen3.8-27B-GSQ-RCO-GGUF) | 採用。既存 Qwen の LM Studio alias に追加 |
| [nvidia/Nemotron-3.5-Parse](https://huggingface.co/nvidia/Nemotron-3.5-Parse) | 保留。専用 OCR / parsing モデルで対象 runtime 名未確認 |
| [huihui-ai/Huihui-Qwen3.8-27B-abliterated-GGUF](https://huggingface.co/huihui-ai/Huihui-Qwen3.8-27B-abliterated-GGUF) | 保留。abliteration で変更されたモデルを元 Qwen に統合しない |
| [meta-llama/Meta-Llama-3-8B-Instruct](https://huggingface.co/meta-llama/Meta-Llama-3-8B-Instruct) | 採用。8B Instruct の固定タグと8192 context を確認 |
| [ISTA-DASLab/Qwen3.8-Flash-Next-GSQ-RCO-Coder-GGUF](https://huggingface.co/ISTA-DASLab/Qwen3.8-Flash-Next-GSQ-RCO-Coder-GGUF) | 保留。llama.cpp の手順はあるが対象 runtime 名は未確認 |
| [JonathanColetti/Qwen3.8-27B-Uncensored-GGUF](https://huggingface.co/JonathanColetti/Qwen3.8-27B-Uncensored-GGUF) | 保留。abliteration / LoRA を適用した別モデルで、元 Qwen に統合しない |
| [ConwayResearch/Underdog-Saluki-27B-1.0](https://huggingface.co/ConwayResearch/Underdog-Saluki-27B-1.0) | 保留。手動 Ollama create の手順はあるが、公開固定タグ・canonical 対応は未確定 |
| [DavidAU/Qwen3.8-27B-Turbo-Brilliance-50X-Reasoning-Instruct-NEO-MAX-MTP-GGUF](https://huggingface.co/DavidAU/Qwen3.8-27B-Turbo-Brilliance-50X-Reasoning-Instruct-NEO-MAX-MTP-GGUF) | 保留。独自 tuning を含む派生で runtime 名未確定 |
| [prism-ml/Ternary-Bonsai-2-27B-gguf](https://huggingface.co/prism-ml/Ternary-Bonsai-2-27B-gguf) | 保留。独自 ternary kernel を要し対象 runtime 名未確認 |
| [baidu/Unlimited-OCR](https://huggingface.co/baidu/Unlimited-OCR) | 保留。community Ollama 配布は存在するが、公式 runtime 名は未確認 |
| [incoai/Qwen3.8-Flash-Next-DFlash2](https://huggingface.co/incoai/Qwen3.8-Flash-Next-DFlash2) | 不採用。独立した target でなく DFlash2 draft |
| [ATH-MaaS/OvisOCR3](https://huggingface.co/ATH-MaaS/OvisOCR3) | 保留。公式手順は vLLM、対象 runtime 名未確認 |
| [DavidAU/Qwen3.5-9B-The-Defiant-Fable-Uncensored-Heretic-NEO-IMATRIX-MAX-MTP-GGUF](https://huggingface.co/DavidAU/Qwen3.5-9B-The-Defiant-Fable-Uncensored-Heretic-NEO-IMATRIX-MAX-MTP-GGUF) | 保留。独自派生モデルで runtime 名未確定 |
| [microsoft/Sentiment-Analysis-7B](https://huggingface.co/microsoft/Sentiment-Analysis-7B) | 保留。Mistral fine-tune で対象 runtime 名未確認 |
| [microsoft/UserRepGen-4B](https://huggingface.co/microsoft/UserRepGen-4B) | 保留。Qwen fine-tune で対象 runtime 名未確認 |
| [nvidia/Muse-Spark-Open-816B-A42B-DFlash2](https://huggingface.co/nvidia/Muse-Spark-Open-816B-A42B-DFlash2) | 不採用。単独実行できない speculative-decoding draft |
| [nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB-LiLiCorr](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB-LiLiCorr) | 不採用。公式カードが standalone でない draft と明記 |
| [nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB-LiLiCorr-GGUF](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Bolt-74B-A9B-24GB-LiLiCorr-GGUF) | 不採用。同上。74B target の alias に統合しない |
| [nvidia/NVIDIA-Nemotron-3.5-Super-VL-120B-A12B-NVFP4](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Super-VL-120B-A12B-NVFP4) | 既存対応。Super VL BF16 と確認済み固定タグは develop に配信済み |
| [nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-BF16](https://huggingface.co/nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-BF16) | 保留。専用 llama.cpp-dory が必要で対象 runtime 名未確認 |
| [nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-GGUF](https://huggingface.co/nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-GGUF) | 保留。Dory の Q5_K_M export、対象 runtime 名未確認 |
| [nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-MXFP8](https://huggingface.co/nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-MXFP8) | 保留。Dory の量子化版で対象 runtime 名未確認 |
| [nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-NVFP4](https://huggingface.co/nvidia/NVIDIA-Nemotron-Labs-Dory1.0-4B-NVFP4) | 保留。Dory の量子化版で対象 runtime 名未確認 |

humanizer の保留判断は [公式 USAGE](https://huggingface.co/jialinyyzz/humanizer/blob/main/USAGE.md)
の runtime 未検証と template 制約による。新しいモデルの検知規模や抽出アルゴリズムの変更は
今回の範囲に含めない。補助重みや低ビット形式のメタデータが原モデルの総数より小さく見える
場合でも、それを採用根拠にしない。

## 検証範囲

`mapping::tests::test_issue_946_*` で双方向の名前解決、engine ごとの範囲、大小文字、
既存 Qwen の優先名維持、context のエンドポイント優先、base / instruction / draft の分離を検証する。
既存カタログの検証と `make quality-checks` も実行し、正式結果は PR の検証記録に残す。

User Verification Result: n/a (autonomous)

Agent Visual Check: n/a (no UI surface)
