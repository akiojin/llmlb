# Nemotron safetensors-cpp PoC

Nemotron 3 Nano 30B A3B(BF16) の safetensors を safetensors-cpp で読み込み、
テンソル名/型/件数の概要を確認するための PoC です。

## 依存
- C++11 以上のコンパイラ
- Hugging Face から safetensors シャードを取得できる環境（`HF_TOKEN` 推奨）

## ビルド

```
$ c++ -std=c++17 -O2 -I poc/nemotron-safetensors-cpp \
  -o poc/nemotron-safetensors-cpp/nemotron_safetensors_poc \
  poc/nemotron-safetensors-cpp/main.cpp
```

## 実行例

Nemotron のシャードは非常に大きい（合計約 63GB）ので、1 シャードだけで検証できます。

```
$ python - <<"PY"
from huggingface_hub import hf_hub_download
path = hf_hub_download(
    repo_id="nvidia/NVIDIA-Nemotron-3-Nano-30B-A3B-BF16",
    filename="model-00001-of-00013.safetensors",
)
print(path)
PY

$ poc/nemotron-safetensors-cpp/nemotron_safetensors_poc \
  /path/to/model-00001-of-00013.safetensors \
  --match experts --limit 5
```

## 出力内容
- 総テンソル数
- dtype 別の件数
- `experts` を含むテンソル件数
- 既知の失敗テンソル（`backbone.layers.1.mixer.experts.0.down_proj.weight`）の有無

## サードパーティ
- safetensors-cpp: MIT License
  - [safetensors-cpp](https://github.com/syoyo/safetensors-cpp)
