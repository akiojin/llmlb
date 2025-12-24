# タスク: 画像生成モデル対応（Image Generation）

**機能ID**: `SPEC-ae3f974e`
**ステータス**: 方針更新（要再設計）
**入力**: `/specs/SPEC-ae3f974e/` の設計ドキュメント

## 更新メモ（共有用）
- 2025-12-24: 画像生成はsafetensors正本、GGUFはsafetensors不在時のみフォールバック。
- Node実行時はPython依存を導入しない。

## TDD順序（必須）
Contract → Integration → E2E → Unit → Core の順で実施する。

## Contract Tests (RED)
- [x] /v0/models/register: `format=safetensors` の必須メタデータ不足で 400 になること（共通契約テスト）。
- [ ] /v1/images/generations: `format=safetensors` の必須メタデータ不足で 400 になること。
- [ ] /v1/images/edits, /v1/images/variations: 画像入力不足やサイズ超過で 400 になること。
- [ ] /v1/models: 実体（safetensors/GGUF）が存在しない画像モデルは表示されないこと。

## Integration Tests (RED)
- [ ] 画像生成モデルが capabilities に応じて正しいノードへルーティングされること。
- [x] safetensors shard 欠損時に Node が未対応として扱うこと（モデル登録統合テストで400）。

## E2E (RED)
- [x] safetensorsモデル登録時にメタデータ不足で 400 になること（共通E2E）。
- [ ] safetensorsモデルで `/v1/images/generations` が非空画像を返すこと。
- [ ] safetensorsモデルで `/v1/images/edits` / `/v1/images/variations` が成功すること。

## Unit Tests (GREEN)
- [ ] Node: 画像モデルの `config.json`/`tokenizer.json` 検証ユニットテスト。
- [ ] Node: safetensors shard 解決ユニットテスト。

## Core
- [ ] Model登録/配布で `format=safetensors` を前提に image runtimes を確定する。
- [ ] Node: 画像生成エンジン（safetensors直読）を実装。
- [ ] Node: GGUFフォールバックは「safetensors不在」の場合のみ許可。
- [ ] Router: `/v0/models/register` で画像モデルの必須ファイル検証を追加。

## Docs
- [ ] README.md / README.ja.md に画像生成モデルの登録・実行要件を追記。

## Deprecated（旧方針・凍結）
- stable-diffusion.cpp + GGML/GGUF 前提の実装タスクは廃止。必要なら新エンジン方針で再起票する。
