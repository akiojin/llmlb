# タスク: 音声モデル対応（TTS + ASR）

**機能ID**: `SPEC-26006000`
**ステータス**: 方針更新（要再設計）
**入力**: `spec.md`, `plan.md`, `data-model.md`, `contracts/`

## 更新メモ（共有用）
- 2025-12-24: ASR/TTSは新エンジンへ置換。Node実行時はPython依存なし。
- safetensorsを正本とし、GGUFはsafetensorsが存在しない場合のみフォールバック。
- Whisper公式 `.pt` はPythonでsafetensors化し、safetensorsを正本とする（取得元: Hugging Face）。

## TDD順序（必須）
Contract → Integration → E2E → Unit → Core の順で実施する。

## Contract Tests (RED)
- [ ] /v1/audio/transcriptions: `format=safetensors` の必須メタデータ不足で 400 になること。
- [ ] /v1/audio/speech: `format=safetensors` の必須メタデータ不足で 400 になること。
- [ ] /v1/models: 実体（safetensors/GGUF）が存在しない音声モデルは表示されないこと。

## Integration Tests (RED)
- [ ] ASR/TTS それぞれの runtime/capabilities に基づき、対応ノードへルーティングされること。
- [ ] safetensors shard 欠損時に Node が未対応として扱うこと。

## E2E (RED)
- [ ] `openai/whisper-*` を safetensors として登録し、`/v1/audio/transcriptions` が非空テキストを返すこと。
- [ ] TTSモデルを safetensors として登録し、`/v1/audio/speech` が音声バイナリを返すこと。

## Unit Tests (GREEN)
- [ ] Node: 音声モデルの `config.json`/`tokenizer.json` 検証ユニットテスト。
- [ ] Node: safetensors shard 解決ユニットテスト。

## Core
- [ ] Model登録/配布で `format=safetensors` を前提に audio runtimes を確定する。
- [ ] Node: ASR/TTS 新エンジン（safetensors直読）を実装。
- [ ] Node: GGUFフォールバックは「safetensors不在」の場合のみ許可。
- [ ] Router: `/v0/models/register` で音声モデルの必須ファイル検証を追加。
- [ ] 変換パイプライン（Python）: Whisper公式 `.pt` → safetensors 変換手順を整備（運用ドキュメント）。

## Docs
- [ ] README.md / README.ja.md に音声モデルの登録・実行要件（safetensors正本、GGUFフォールバック条件）を追記。

## Deprecated（旧方針・凍結）
- whisper.cpp/ONNX Runtime前提の実装タスクは廃止。必要なら新エンジン方針で再起票する。
