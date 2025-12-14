# VibeVoice-Realtime-0.5B PyTorch PoC (ローカル推論用)

目的: llama.cpp では扱えない VibeVoice-Realtime-0.5B を PyTorch で直接動かす最小手順を示す。  
この PoC は **実行はデフォルトで行わず**、ユーザが明示的に実行する想定。M4 環境では CPU 実行になる（bfloat16 を使うが速度は要覚悟）。

## 必要環境
- Python 3.11 以上推奨
- `pip install -r requirements.txt`
- 事前に `HF_TOKEN` を環境変数でセットするとダウンロードが安定

## 実行例
```bash
cd poc/vibevoice-pytorch
HF_TOKEN=xxx python3 run.py --text "Hello from VibeVoice on PyTorch."
```

デフォルトでは:
- モデル: `microsoft/VibeVoice-Realtime-0.5B`
- 出力 WAV: `out.wav`
- デバイス: CPU（M4ではCUDA/ROCmなし）

## 注意点
- VibeVoice は音響トークナイザ＋拡散ヘッドを含むカスタム実装で、Transformers 標準の ONNX エクスポート非対応。ONNX 化する場合は各ブロック（音響トークナイザ / LLM / 拡散ヘッド）を分割して自前でエクスポート・統合する必要がある。
- このスクリプトは **バッチ/ストリーミング最適化なし** の素朴な実行。レイテンシは実用水準ではなく、PoCとして「音が出る」確認が目的。
- メモリ使用量は数 GB 程度を想定。モバイル Mac ではスワップに注意。
