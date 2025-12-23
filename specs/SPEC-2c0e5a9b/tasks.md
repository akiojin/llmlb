# SPEC-2c0e5a9b: Tasks

## Setup
- [x] 「公式最適化アーティファクト」許可リスト初期値を確定（`openai/*`, `nvidia/*`）

## Core
- [x] Node: `config.json` から gpt-oss runtime を検出（`gptoss_cpp`）
- [x] Node: gpt-oss 用エンジン（Metal: 公式 `model.bin`）のスケルトンを追加し EngineRegistry に登録
- [x] Router: 公式 Metal アーティファクト `metal/model.bin` を `model.metal.bin` として任意取得（allowlist）
- [x] Router: registry manifest に `runtimes` ヒントを付与（Node が未対応モデルの取得をスキップ可能）
- [x] Node: registry manifest の `runtimes` を見て未対応モデルのダウンロードをスキップ

## Tests
- [x] Node: `config.json` から gpt-oss runtime を検出するユニットテストを追加
- [x] Node: unit/integration/contract を含む CMake ビルドが通ることを確認

## Deferred（別SPEC想定 / 本SPECでは後回し）
- CUDA backend（公式最適化アーティファクトの有無調査 + 実装方針）
- safetensors（index+shards）を直接実行する gpt-oss 推論（Metal/CUDA 共通）
- gpt-oss-20b 実モデルでの `/v1/chat/completions` 統合テスト（サイズが大きいため別途）
- README.md / README.ja.md の運用追記
