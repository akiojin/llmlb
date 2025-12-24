# SPEC-2c0e5a9b: Plan

## 方針
- safetensors を正本とし、Node 側で “推論エンジン（線）” を追加して gpt-oss-20b を GPU 実行する。
- エンジン選択は既存の抽象化（`SPEC-d7feaa2c`）を利用し、登録時の `format` と `config.json` 等の HF 由来メタデータに従う。
- `chat_template` の解釈は C++ Node に寄せず、Router 側で Jinja 互換レンダリングを行い、Node には最終プロンプト（テキスト）を渡す方針を前提とする。
- Node は Python 依存なしで動作する（必須）。
- Nemotron 推論エンジンは本件では扱わない（別SPECで後日）。
- 実行の優先順位:
  1) 公式のGPU最適化アーティファクト（許可リスト対象、実行キャッシュ）
  2) safetensors（正本）

## 対象モデルとアーティファクト（前提）
- 対象: `openai/gpt-oss-20b`
- 前提（HFスナップショット）:
  - `config.json`, `tokenizer.json`（必須）
  - `model.safetensors.index.json` + `model-*.safetensors`（シャーディング）
  - `chat_template.jinja`（任意）
- 備考: モデルによっては公式のGPU最適化アーティファクト（例: `metal/model.bin`）が提供される場合がある。
  - safetensors は常に正本として保持する。
  - 実行は「公式最適化アーティファクトが利用可能なら優先、無ければ safetensors」を基本とする（登録形式は変えない）。

## 実装スコープ（設計）

### Router（登録・配布）
- `format=safetensors` 登録時:
  - 必須メタデータ検証（`config.json`, `tokenizer.json`）
  - index/shards の整合検証（欠損があれば失敗）
  - Node が必要とするファイル一覧（マニフェスト）を確定
  - 公式のGPU最適化アーティファクトが利用可能な場合は**実行キャッシュとして**マニフェストへ含める（登録形式は変えない）
- `chat_template` が無い場合のデフォルトテンプレートを提供

### Node（取得・検証・実行）
- ModelStorage:
  - gpt-oss を `config.json` から検出し、対応 runtime を決定できる
  - safetensors（index + shards）を 1 モデルとして検証できる
- Engine:
  - gpt-oss 用 runtime を追加し、GPU 実行（Metal/CUDA）を提供する
  - 公式最適化アーティファクトがローカルにある場合はそれを優先してロードする
  - 対応不可の場合は明確に未対応として扱い、ready 一覧から除外できる

## 決定事項（設計合意）
- 「公式最適化アーティファクト」は、同一 publisher org（例: `openai`, `nvidia`）配下の別リポジトリに存在してよい。
- 取得元は許可リストで管理する（許可リスト外は無視）。
- 許可リスト初期値: `openai/*`, `nvidia/*`
- 登録形式は常に `format=safetensors` を維持し、公式最適化アーティファクトは実行キャッシュとして扱う。

## 主要な要明確化（実装前に決めること）
- CUDA 実行の実現方法（Python なしで成立させる手段）。
- 公式GPU最適化アーティファクトの「自動利用 / 明示 opt-in」方針。

## テスト方針（TDD）
- Contract: Router API（登録/一覧）と Node API（chat/completions）の契約を増やす
- Integration: gpt-oss-20b を `format=safetensors` で登録 → Node がロード → 生成成功、を最小経路で確認
- E2E: ダッシュボードからの登録 → チャット疎通（可能なら）

## ドキュメント
- README に「safetensorsを正本として登録する」「gpt-oss-20b の前提ファイル」「未対応時の挙動」を追記する。
