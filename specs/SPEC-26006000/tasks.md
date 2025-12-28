# タスク: 音声モデル対応（TTS + ASR）

**機能ID**: `SPEC-26006000`
**ステータス**: 方針更新（要再設計）
**入力**: `spec.md`, `plan.md`, `data-model.md`, `contracts/`

## 更新メモ（共有用）
- 2025-12-24: ASRはwhisper.cpp（GGML/GGUF運用）で当面対応、
  TTSはONNX Runtimeを利用（Node実行時はPython依存なし）。
- safetensorsを正本とし、safetensors/GGUFが共存する場合は登録時にformat指定必須。
- Whisper公式 `.pt` はPythonでsafetensors化し、safetensorsを正本とする（取得元: Hugging Face）。

## TDD順序（必須）
Contract → Integration → E2E → Unit → Core の順で実施する。

## Contract Tests (RED)
- [x] /v1/audio/transcriptions: 形式/必須パラメータ/認証の契約テスト（既存）。
- [x] /v1/audio/speech: 形式/必須パラメータ/認証の契約テスト（既存）。
- [x] /v0/models/register: safetensorsメタデータ必須の契約テスト（共通）。
- [ ] /v1/models: 音声モデルに speech_to_text / text_to_speech のcapabilityが表示されること。

## Integration Tests (RED)
- [x] ASR/TTS それぞれの runtime/capabilities に基づき、対応ノードへルーティングされること。
- [x] safetensors shard 欠損時に登録が拒否されること（統合テスト）。

## E2E (RED)
- [x] `openai/whisper-*` を safetensors として登録した際、メタデータ不足で 400 になること（共通E2E）。
- [x] `openai/whisper-*` を登録し、`/v1/audio/transcriptions` が非空テキストを返すこと（TDD RED: ignored）。
- [x] TTSモデルを登録し、`/v1/audio/speech` が音声バイナリを返すこと（TDD RED: ignored）。

## Unit Tests (GREEN)
- [ ] Node: 音声モデルの `config.json`/`tokenizer.json` 検証ユニットテスト。
- [ ] Node: safetensors shard 解決ユニットテスト。

## Core
- [ ] Model登録/配布で audio runtimes を確定する（format=gguf/onnx等）。
- [ ] Node: ASRはwhisper.cpp（GGML/GGUF運用）を統合する。
- [ ] Node: TTSはONNX Runtimeを統合する。
- [ ] Node: safetensors/GGUF共存時は登録時formatに従い、実行時フォールバックを禁止。
- [ ] Router: `/v0/models/register` で音声モデルの必須ファイル検証を追加。
- [ ] 変換パイプライン（Python）: Whisper公式 `.pt` → safetensors 変換手順を整備（運用ドキュメント）。

## Docs
- [ ] README.md / README.ja.md に音声モデルの登録・実行要件（safetensors正本、GGUF選択条件）を追記。

## Deprecated（旧方針・凍結）
- ASR/TTSを独自エンジンで実装する前提のタスクは廃止。
