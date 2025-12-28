# タスク: 画像生成モデル対応（Image Generation）

**機能ID**: `SPEC-ae3f974e`
**ステータス**: 方針更新（要再設計）
**入力**: `/specs/SPEC-ae3f974e/` の設計ドキュメント

## 更新メモ（共有用）
- 2025-12-24: 画像生成はsafetensors正本、safetensors/GGUF共存時は登録時にformat指定必須。
- Node実行時はPython依存を導入しない。

## TDD順序（必須）
Contract → Integration → E2E → Unit → Core の順で実施する。

## Contract Tests (RED)
- [x] /v1/images/generations: 必須パラメータ/認証/ノード不在の契約テスト（既存）。
- [x] /v1/images/edits, /v1/images/variations: 画像入力不足/認証/ノード不在の契約テスト（既存）。
- [x] /v0/models/register: safetensorsメタデータ必須の契約テスト（共通）。
- [ ] /v1/models: image_generation capability が表示されること。

## Integration Tests (RED)
- [x] 画像生成モデルが capabilities に応じて正しいノードへルーティングされること。
- [x] safetensors shard 欠損時に登録が拒否されること（統合テスト）。

## E2E (RED)
- [x] safetensorsモデル登録時にメタデータ不足で 400 になること（共通E2E）。
- [x] safetensorsモデルで `/v1/images/generations` が非空画像を返すこと（TDD RED: ignored）。
- [x] safetensorsモデルで `/v1/images/edits` / `/v1/images/variations` が成功すること（TDD RED: ignored）。

## Unit Tests (GREEN)
- [ ] Node: 画像モデルの `config.json`/`tokenizer.json` 検証ユニットテスト。
- [ ] Node: safetensors shard 解決ユニットテスト。

## Core
- [ ] Model登録/配布で format に従い image runtimes を確定する。
- [ ] Node: 画像生成エンジン（safetensors直読）を実装。
- [ ] Node: safetensors/GGUF共存時は登録時formatに従い、実行時フォールバックを禁止。
- [ ] Router: `/v0/models/register` で画像モデルの必須ファイル検証を追加。

## Docs
- [ ] README.md / README.ja.md に画像生成モデルの登録・実行要件を追記。

## Deprecated（旧方針・凍結）
- stable-diffusion.cpp + GGML/GGUF 前提の実装タスクは廃止。必要なら新エンジン方針で再起票する。
