# SPEC-2c0e5a9b: Tasks

## 更新メモ（共有用）
- 2025-12-24: gpt-oss-20b は **safetensors 直読エンジンを主経路**とし、公式GPU最適化アーティファクトは**実行キャッシュ**として扱う。
- 2025-12-28: 対応OS/GPUを **macOS=Metal / Windows=CUDA** に変更。Linuxは当面非対応（CUDAは実験扱い）。

## TDD順序（必須）
- Contract → Integration → E2E → Unit → Core/Refactor の順で実施する。

## Contract Tests (RED)
- [x] Router: gpt-oss 登録時に `format=safetensors` 必須メタデータ欠落が 400 になること。
- [x] Router: `model.safetensors.index.json` が無い状態で複数 shard を検出した場合に 400 になること。
- [x] Node: gpt-oss runtime 判定が `config.json` から確定すること（既存テストの拡張）。

## Integration Tests (RED)
- [x] Node: sharded safetensors の欠損 shard を検出してロード失敗すること。
- [x] Node: 必須メタデータ（config/tokenizer）不足時に未対応として扱うこと。

## E2E (RED)
- [x] gpt-oss-20b を `format=safetensors` で登録 → 配布 → `/v1/chat/completions` が 1 token 以上生成すること。

## Core
- [x] Node: safetensors（index + shards）を 1 モデルとしてロードする実装（メタデータ検証込み）。
- [x] Node: EngineRegistry/TextManager で gpt-oss runtime を解決できるようにする。
- [x] Node: gpt-oss safetensors 推論パス（Metal/CUDA）を内蔵エンジンとして実装する。
- [x] Node: KVキャッシュ/サンプリングを含む最小生成ループを実装。
- [x] Router: gpt-oss safetensors の必須ファイル群を manifest に確定する。
- [x] Router: 公式GPU最適化アーティファクトを **マニフェストに含める**（取得はNode主導、supported_models.json の artifacts 指定）。
- [x] Node: DirectML 最適化アーティファクト（model.directml.bin / model.dml.bin）をロード対象として扱う。
- [x] Node: DirectML ランタイム DLL を動的ロードし、未配置時は明示エラーを返す。
- [x] Node: DirectML 向け gpt-oss ランタイムの最小スケルトンを追加。

## Unit Tests (GREEN)
- [x] Node: safetensors shards 解決とメタデータ検証のユニットテスト。
- [x] Node: gpt-oss 推論パスの最小ユニットテスト（CPU/Stub不可、GPU実行環境で検証）。
- [x] Node: manifest の optional ファイルは取得失敗でも継続できるユニットテスト。
- [x] Node: runtime メタデータ/設定の検証ユニットテスト。
- [x] Node: DirectML ランタイム未配置時のロード失敗を検証するユニットテスト。

## Docs
- [x] README.md / README.ja.md に gpt-oss safetensors 前提と実行要件を追記。
- [x] Docs: DirectML ランタイムの配布元（GitHub Releases）を明記。

## Exploratory (既存作業の記録)
- [x] Node: `config.json` から gpt-oss runtime を検出（`gptoss_cpp`）
- [x] Node: gpt-oss 用エンジン（Metal: 公式 `model.bin`）のスケルトンを追加し EngineRegistry に登録
- [x] Router: 公式 Metal アーティファクト `metal/model.bin` を `model.metal.bin` として任意取得（allowlist）
- [x] Router: registry manifest に `runtimes` ヒントを付与（Node が未対応モデルの取得をスキップ可能）
- [x] Node: registry manifest の `runtimes` を見て未対応モデルのダウンロードをスキップ

## ブロッカー
- gpt-oss safetensors 推論（Metal/CUDA）は GPU 実行環境と USE_GPTOSS ビルドが必要
- CUDA 実装/検証には Windows + NVIDIA GPU + CUDA 実行環境が必要
