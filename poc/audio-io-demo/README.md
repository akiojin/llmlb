# Audio I/O PoC (ASR + TTS)

目的: Node の音声APIの **入力（ASR）** と **出力（TTS）** を、最低限の形でエンドツーエンドに確認します。

- ASR: `/v1/audio/transcriptions`（whisper.cpp）
- TTS: `/v1/audio/speech`（ONNX Runtime）

注意:
- デフォルトのTTSは macOS 標準の `say` を使い、**人の声**で読み上げます（`TTS_MODEL=macos_say`）。
- `TTS_MODEL=vibevoice` を指定すると **VibeVoice-Realtime-0.5B (PyTorch/MPS)** で音声生成します（初回は venv 作成＋依存導入＋モデルDLで時間がかかります）。
- `TTS_MODEL=toy_tts.onnx` を指定すると toy ONNX モデルで **短いビープ音** を返します（音が返ることの最低限確認用）。
- GPU前提: onnxruntime は CoreML EP を想定し、CPUフォールバック無しで実行します（LLM/ONNX系）。

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

デフォルトでは `afplay` で自動再生します（無効にしたい場合は `PLAY_TTS=0`）。

```bash
PLAY_TTS=0 ./poc/audio-io-demo/run_audio_io_poc.sh
```

環境変数で出力先などを変更できます:

```bash
MODEL_DIR=/tmp/llm_router_audio_poc_models ./poc/audio-io-demo/run_audio_io_poc.sh
```

ASRの入力WAVは `ASR_WAV_PATH` で差し替えできます（デフォルトは `node/third_party/whisper.cpp/samples/jfk.wav`）。
言語は `ASR_LANGUAGE` で指定できます（デフォルト: `en`、自動検出したい場合は `auto`）。

`ASR_WAV_PATH` は `.m4a` 等でも指定できます。macOS 標準の `afconvert` で入力を
`16kHz / mono / 16-bit PCM WAV` に正規化してから送信します（node 側は現状 WAV のみ対応）。

TTSの入力テキストはデフォルトで「ASRの結果テキスト」を使います。上書きしたい場合は `TTS_TEXT` を指定してください。

```bash
TTS_TEXT="こんにちは。これは音声IO PoCです。" ./poc/audio-io-demo/run_audio_io_poc.sh
```

macOS `say` の音声（voice）を変えたい場合は `TTS_VOICE` を指定してください（例: `Kyoko`）。

```bash
say -v '?'
TTS_VOICE="Kyoko" ./poc/audio-io-demo/run_audio_io_poc.sh
```

VibeVoice を使う場合:

```bash
TTS_MODEL=vibevoice ./poc/audio-io-demo/run_audio_io_poc.sh
```

`VIBEVOICE_VENV_DIR` に venv を作って依存をインストールします。デバイスは `VIBEVOICE_DEVICE` で指定できます（デフォルト: `mps`）。

```bash
VIBEVOICE_DEVICE=mps TTS_MODEL=vibevoice ./poc/audio-io-demo/run_audio_io_poc.sh
```

VibeVoice は voice sample が必須です。PoCではデフォルトで **ASR入力音声をそのまま voice sample として使用**します。
別の音声を使いたい場合は `TTS_VOICE` にファイルパス（`.wav` / `.m4a` など）を指定してください。

```bash
TTS_MODEL=vibevoice TTS_VOICE="/path/to/voice_sample.wav" ./poc/audio-io-demo/run_audio_io_poc.sh
```

`TTS_MODEL=toy_tts.onnx` を使う場合のみ、Python依存（`onnx`, `numpy`）をスクリプト内で `venv` にインストールします（Homebrewの `externally-managed-environment` 回避）。
必要に応じて `VENV_DIR` / `PYTHON_BIN` で変更できます。
