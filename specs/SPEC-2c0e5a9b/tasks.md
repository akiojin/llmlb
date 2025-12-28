# SPEC-2c0e5a9b: Tasks

## 更新メモ（共有用）
- 2025-12-24: gpt-oss-20b は **safetensors 直読エンジンを主経路**とし、公式GPU最適化アーティファクトは**実行キャッシュ**として扱う。
- 既存の `metal/model.bin` 系は**探索的実装**として記録し、本番要件は safetensors 直読に置き換える。

## TDD順序（必須）
- Contract → Integration → E2E → Unit → Core/Refactor の順で実施する。

## Contract Tests (RED)
- [x] Router: gpt-oss 登録時に `format=safetensors` 必須メタデータ欠落が 400 になること。
- [x] Router: `model.safetensors.index.json` が無い状態で複数 shard を検出した場合に 400 になること。
- [ ] Node: gpt-oss runtime 判定が `config.json` から確定すること（既存テストの拡張）。

## Integration Tests (RED)
- [x] Node: sharded safetensors の欠損 shard を検出してロード失敗すること（REDテスト追加済み、実装待ち）。
- [ ] Node: 必須メタデータ（config/tokenizer）不足時に未対応として扱うこと。

## E2E (RED)
- [ ] gpt-oss-20b を `format=safetensors` で登録 → 配布 → `/v1/chat/completions` が 1 token 以上生成すること。

## Core
- [ ] Node: safetensors（index + shards）を 1 モデルとしてロードする実装（メタデータ検証込み）。
- [x] Node: Engine Host（プラグインローダー）で gpt-oss plugin をロードできるようにする。
- [ ] Node: gpt-oss safetensors 推論パス（Metal/CUDA）を plugin として実装する。
- [ ] Node: KVキャッシュ/サンプリングを含む最小生成ループを実装。
- [ ] Router: gpt-oss safetensors の必須ファイル群を manifest に確定する。
- [ ] Router: 公式GPU最適化アーティファクトが許可リスト内なら**実行キャッシュとして**取得できる導線を用意（自動/opt-inは plan.md の決定に従う）。

## Unit Tests (GREEN)
- [ ] Node: safetensors shards 解決とメタデータ検証のユニットテスト。
- [ ] Node: gpt-oss 推論パスの最小ユニットテスト（CPU/Stub不可、GPU実行環境で検証）。
- [x] Node: プラグイン manifest/ABI の検証ユニットテスト。

## Docs
- [ ] README.md / README.ja.md に gpt-oss safetensors 前提と実行要件を追記。

## Exploratory (既存作業の記録)
- [x] Node: `config.json` から gpt-oss runtime を検出（`gptoss_cpp`）
- [x] Node: gpt-oss 用エンジン（Metal: 公式 `model.bin`）のスケルトンを追加し EngineRegistry に登録
- [x] Router: 公式 Metal アーティファクト `metal/model.bin` を `model.metal.bin` として任意取得（allowlist）
- [x] Router: registry manifest に `runtimes` ヒントを付与（Node が未対応モデルの取得をスキップ可能）
- [x] Node: registry manifest の `runtimes` を見て未対応モデルのダウンロードをスキップ
