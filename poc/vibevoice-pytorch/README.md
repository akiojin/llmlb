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
pip install --no-deps git+https://github.com/microsoft/VibeVoice.git
```

## 実行例
```bash
cd poc/vibevoice-pytorch
HF_TOKEN=xxx python3 run.py --require-gpu --device mps --voice Carter --text "Hello from VibeVoice on PyTorch."
```

デフォルトでは:
- モデル: `microsoft/VibeVoice-Realtime-0.5B`
- 出力 WAV: `out.wav`
- デバイス: 自動選択（`cuda`→`mps` の順。`--require-gpu` を付けると GPU が無い場合は失敗）
- voice: `Carter`（VibeVoice 公式の embedded voice prompt を自動DLして使用）

## 注意点
- VibeVoice は音響トークナイザ＋拡散ヘッドを含むカスタム実装で、Transformers 標準の ONNX エクスポートは難しい（ブロック分割＋独自統合が必要）。
- 生成品質/速度は `--device` / `--ddpm-steps` / `--cfg-scale` に依存。Apple Silicon の場合は `mps` 推奨。
- Realtime モデルは voice sample（wav等）ではなく、`.pt`（埋め込み形式の voice prompt）を使用します。

## よく使うオプション
```bash
# MPS 固定 + GPU必須
python3 run.py --device mps --require-gpu --voice Carter --ddpm-steps 5 --cfg-scale 1.5 --text "Hello!"

# 日本語ボイス preset（音声品質は入力言語に依存します）
python3 run.py --device mps --require-gpu --voice jp-Spk1_woman --text "こんにちは。これはVibeVoiceのPoCです。"

# `.pt` voice prompt をローカル指定
python3 run.py --device mps --require-gpu --voice /path/to/en-Emma_woman.pt --text "Hello!"
```
