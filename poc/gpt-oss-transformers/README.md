# gpt-oss-20b Transformers PoC (macOS / Apple Silicon)

目的: `openai/gpt-oss-20b` を **Transformers で直接**動かして、テキスト生成が成立するかを確認する PoC。

注意: この PoC は **ONNX ではありません**（gpt-oss 公式が案内する Transformers 実行経路の確認用）。

## 必要環境
- Python 3.10+ 推奨（venv 推奨）
- `HF_TOKEN` を設定するとダウンロードが安定

## セットアップ
```bash
cd poc/gpt-oss-transformers
python3 -m venv .venv
source .venv/bin/activate
pip install -U pip
pip install -r requirements.txt
```

## 実行例
```bash
HF_TOKEN=xxx python3 run.py --prompt "Explain quantum mechanics clearly and concisely."
```

オプション:
```bash
# CPU 固定（検証用。かなり遅い可能性あり）
python3 run.py --device cpu --max-new-tokens 128
```

