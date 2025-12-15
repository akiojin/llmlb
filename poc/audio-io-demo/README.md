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

既に別プロセスの `llm-node` / `router` がポートを使用している場合、PoC は自動で空きポートにずらします。
固定したい場合は `NODE_PORT` / `ROUTER_PORT` を指定してください。

```bash
NODE_PORT=11445 ROUTER_PORT=28080 ./poc/audio-io-demo/run_audio_io_poc.sh
```

出力物:
- ASR: JSON（`{"text": ...}`）
- TTS: `MODEL_DIR/tts_out.wav`（WAV）

macOSでの再生例:

```bash
afplay /tmp/llm_router_audio_poc_models/tts_out.wav
```

自動再生したい場合:

```bash
PLAY_TTS=1 ./poc/audio-io-demo/run_audio_io_poc.sh
```

環境変数で出力先などを変更できます:

```bash
MODEL_DIR=/tmp/llm_router_audio_poc_models ./poc/audio-io-demo/run_audio_io_poc.sh
```

ASRの入力WAVは `ASR_WAV_PATH` で差し替えできます（デフォルトは `node/third_party/whisper.cpp/samples/jfk.wav`）。
言語は `ASR_LANGUAGE` で指定できます（デフォルト: `en`、自動検出したい場合は `auto`）。

`ASR_WAV_PATH` は `.m4a` 等でも指定できます。macOS 標準の `afconvert` で入力を
`16kHz / mono / 16-bit PCM WAV` に正規化してから送信します（node 側は現状 WAV のみ対応）。

Python依存（`onnx`, `numpy`）はスクリプト内で `venv` を作ってインストールします（Homebrewの `externally-managed-environment` 回避）。
必要に応じて `VENV_DIR` / `PYTHON_BIN` で変更できます。
