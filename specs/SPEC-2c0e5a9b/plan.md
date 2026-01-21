# SPEC-2c0e5a9b: Plan

## 方針
- gpt-oss 用 runtime を内蔵ランタイムとして追加し、GPU 実行（Metal/CUDA）を提供する。
- エンジン選択は既存の抽象化（`SPEC-d7feaa2c`）を利用し、登録時の `format` と HF 由来メタデータ（`config.json` 等）に従う。
- `chat_template` の解釈は Router 側で行い、Node には最終プロンプト（テキスト）を渡す。
- Node は Python 依存なしで動作する（必須）。
- Nemotron 推論エンジンは本件では扱わない（別 SPEC で後回し）。
- 公式 GPU 最適化アーティファクトがある場合は「実行キャッシュ」として優先利用する。
- 対応 OS/GPU は macOS=Metal / Windows=CUDA。Linux は非対応、DirectML は凍結。

## 対象モデル / アーティファクト
- 対象: `openai/gpt-oss-20b`
- 必須ファイル
  - `config.json`, `tokenizer.json`
  - `model.safetensors.index.json` + `model-*.safetensors`（シャーディング）
  - `chat_template.jinja`（任意）
- 公式最適化アーティファクト（任意）
  - Metal: `model.metal.bin`
  - CUDA: `model.cuda.bin` または `cuda/model.bin`

## 実装スコープ

### Router（登録・配布）
- `format=safetensors` での登録を前提。
- `config.json` / `tokenizer.json` / index+shards の整合性検証。
- Node が必要とするファイル一覧（manifest）を生成。
- 公式最適化アーティファクトが利用可能な場合は manifest に含める（登録形式は変えない）。
- 取得元は許可リストで制御（例: `openai/*`, `nvidia/*`）。

### Node（取得・検証・実行）
- ModelStorage: safetensors の必須ファイル検証。
- ModelResolver/Sync: GPU バックエンドに応じて最適化アーティファクトを優先選択。
- EngineRegistry/EngineHost: `GptOssCpp` を解決し、対応不可なら ready 対象から除外。
- エラー: DLL 未配置 / アーティファクト欠落を明確に報告。
- GPU 前提: macOS=Metal / Windows=CUDA、Linux/WSL2 は対象外。

### Engine（gpt-oss runtime）
- 公式最適化アーティファクトがある場合はそれを優先ロード。
- 無い場合は safetensors をロード。
- Windows CUDA は `gptoss_cuda.dll` の存在を必須とする。

## テスト
- Unit: アーティファクト選択（Metal/CUDA）、DLL 不足のエラー確認。
- Integration: safetensors 登録 → ready 表示（Metal/CUDA）。
- E2E: `POST /v1/chat/completions` の疎通（通常/ストリーミング）。

## ドキュメント
- Quickstart に必要ファイル・環境変数・アーティファクトの配置例を追記。
- DirectML は凍結、Windows は CUDA 主経路であることを明記。

## 未確定事項
- CUDA DLL の提供元 / ビルド手順。
- CUDA 最適化アーティファクトの生成 / 配布経路。
