# VibeVoice-Realtime-0.5B PyTorch PoC (ローカル推論用)

目的: ONNX への変換が難しい VibeVoice-Realtime-0.5B を **PyTorch で直接**動かし、
「音が出る」ことを確認する PoC。

この PoC は **実行はデフォルトで行わず**、ユーザが明示的に実行する想定。

## 必要環境
- Python 3.9+（推奨: 3.11+）
- venv 推奨（依存が重いのでグローバル汚染を避ける）
- 事前に `HF_TOKEN` を環境変数でセットするとダウンロードが安定

```bash
cd poc/vibevoice-pytorch
python3 -m venv .venv
source .venv/bin/activate
pip install -U pip
pip install -r requirements.txt
```

## 実行例
```bash
cd poc/vibevoice-pytorch
HF_TOKEN=xxx python3 run.py --text "Hello from VibeVoice on PyTorch."
```

デフォルトでは:
- モデル: `microsoft/VibeVoice-Realtime-0.5B`
- 出力 WAV: `out.wav`
- デバイス: 自動選択（`cuda`→`mps`→`cpu` の順）
- voice prompt: Microsoft/VibeVoice の GitHub から `.pt` を初回だけダウンロードしてキャッシュ

## 注意点
- VibeVoice は音響トークナイザ＋拡散ヘッドを含むカスタム実装で、Transformers 標準の ONNX エクスポートは難しい（ブロック分割＋独自統合が必要）。
- `vibevoice` パッケージは依存が重い（`diffusers` / `gradio` 等も入る）。PoC用途で割り切る。
- 生成品質/速度は `--device` / `--ddpm-steps` / `--cfg-scale` に依存。M4 の場合は `mps` 推奨。

## よく使うオプション
```bash
# MPS 固定 + 速め
python3 run.py --device mps --ddpm-steps 5 --cfg-scale 1.5 --text "Hello!"

# voice prompt を指定（プリセット名 or ローカルパス）
python3 run.py --voice en-Carter_man --text "Hello!"
python3 run.py --voice /path/to/en-Carter_man.pt --text "Hello!"
```
