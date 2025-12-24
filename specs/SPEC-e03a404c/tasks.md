# タスク: 画像認識モデル対応（Image Understanding）

**機能ID**: `SPEC-e03a404c`
**ステータス**: 方針更新（要再設計）
**入力**: `/specs/SPEC-e03a404c/` の設計ドキュメント

## 更新メモ（共有用）
- 2025-12-24: 画像認識はsafetensors正本、GGUFはsafetensors不在時のみフォールバック。
- Node実行時はPython依存を導入しない。

## TDD順序（必須）
Contract → Integration → E2E → Unit → Core の順で実施する。

## Contract Tests (RED)
- [ ] /v1/chat/completions: 画像URL/Base64付きリクエストの契約テスト。
- [ ] Vision非対応モデルに画像付きリクエストを送ると 400 になること。
- [ ] /v1/models: `image_understanding` capability が表示されること。

## Integration Tests (RED)
- [ ] 画像認識モデルが capabilities に応じて正しいノードへルーティングされること。
- [ ] safetensors shard 欠損時に Node が未対応として扱うこと。

## E2E (RED)
- [ ] safetensorsモデルで画像付き `/v1/chat/completions` が非空テキストを返すこと。
- [ ] 複数画像（最大10枚）が処理できること。

## Unit Tests (GREEN)
- [ ] Router: 画像URL取得・Base64デコード・サイズ制限のユニットテスト。
- [ ] Node: 画像認識モデルの `config.json`/`tokenizer.json` 検証ユニットテスト。

## Core
- [ ] Model登録/配布で `format=safetensors` を前提に vision runtimes を確定する。
- [ ] Router: 画像付きchatリクエストのパース/検証を実装。
- [ ] Node: 画像認識エンジン（safetensors直読）を実装。
- [ ] Node: GGUFフォールバックは「safetensors不在」の場合のみ許可。

## Docs
- [ ] README.md / README.ja.md に画像認識モデルの登録・実行要件を追記。

## Deprecated（旧方針・凍結）
- llama.cpp multimodal 前提の実装タスクは廃止。必要なら新エンジン方針で再起票する。
