# Audio I/O PoC (ASR + TTS)

目的: Node の音声APIの **入力（ASR）** と **出力（TTS）** を、最低限の形でエンドツーエンドに確認します。

- ASR: `/v1/audio/transcriptions`（whisper.cpp）
- TTS: `/v1/audio/speech`（ONNX Runtime）

注意:
- このPoCのTTSは「音が返る」ことの確認を目的にした **toyモデル** を使います（人間が聞いて意味のある音声品質は目的外）。
- GPU前提: onnxruntime は CoreML EP を想定し、CPUフォールバック無しで実行します。

## 実行（macOS）

```bash
./poc/audio-io-demo/run_audio_io_poc.sh
```

出力物:
- ASR: JSON（`{"text": ...}`）
- TTS: `MODEL_DIR/tts_out.wav`（WAV）

環境変数で出力先などを変更できます:

```bash
MODEL_DIR=/tmp/llm_router_audio_poc_models ./poc/audio-io-demo/run_audio_io_poc.sh
```

ASRの入力WAVは `ASR_WAV_PATH` で差し替えできます（デフォルトは `node/third_party/whisper.cpp/samples/jfk.wav`）。
言語は `ASR_LANGUAGE` で指定できます（デフォルト: `en`、自動検出したい場合は `auto`）。

Python依存（`onnx`, `numpy`）はスクリプト内で `venv` を作ってインストールします（Homebrewの `externally-managed-environment` 回避）。
必要に応じて `VENV_DIR` / `PYTHON_BIN` で変更できます。
