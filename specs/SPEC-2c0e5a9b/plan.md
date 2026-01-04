# SPEC-2c0e5a9b: Plan

## 方針

- gpt-oss 用 runtime をプラグインとして追加し、GPU 実行（Metal/CUDA）を提供する
- エンジン選択は既存の抽象化（`SPEC-d7feaa2c`）を利用し、登録時の `format` と `config.json` 等の HF 由来メタデータに従う。
- gpt-oss 実行エンジンは **プラグイン形式（動的ロード）** で提供する。
- `chat_template` の解釈は C++ Node に寄せず、Router 側で Jinja 互換レンダリングを行い、Node には最終プロンプト（テキスト）を渡す方針を前提とする。
- Node は Python 依存なしで動作する（必須）。
- Nemotron 推論エンジンは本件では扱わない（別SPECで後日）。
- 実行の優先順位:
  - 公式のGPU最適化アーティファクト（バックエンド依存、許可リスト対象）
  - safetensors（正本）
- 対応OS/GPU: macOS=Metal、Windows=CUDA、Linuxは非対応。DirectMLは実験扱い。

## 対象モデルとアーティファクト（前提）
- 対象: `openai/gpt-oss-20b`
- 前提（HFスナップショット）:
  - `config.json`, `tokenizer.json`（必須）
  - `model.safetensors.index.json` + `model-*.safetensors`（シャーディング）
  - `chat_template.jinja`（任意）
- 備考: モデルによっては公式のGPU最適化アーティファクト（例: `metal/model.bin`）が提供される場合がある。
  - safetensors は常に正本として保持する。
- 実行は「バックエンドに一致する公式最適化アーティファクトが利用可能なら優先、無ければ safetensors」を基本とする（登録形式は変えない）。

## 実装スコープ（設計）

### Router（登録・配布）
- `format=safetensors` 登録時:
  - 必須メタデータ検証（`config.json`, `tokenizer.json`）
  - index/shards の整合検証（欠損があれば失敗）
- Node が必要とするファイル一覧（マニフェスト）を確定
- 公式のGPU最適化アーティファクトが利用可能な場合は**実行キャッシュとして**マニフェストへ含める（登録形式は変えない）
  - 追加アーティファクトは対応モデル定義（supported_models.json の artifacts）で指定する
- ルーターは事前キャッシュ前提を廃止し、**マニフェストの提示のみ**を担当する（取得はNode主導）
- `chat_template` が無い場合のデフォルトテンプレートを提供

### Node（取得・検証・実行）
- ModelStorage:
  - gpt-oss を `config.json` から検出し、対応 runtime を決定できる
  - safetensors（index + shards）を 1 モデルとして検証できる
- Engine:
  - gpt-oss 用 runtime をプラグインとして追加し、GPU 実行（Metal/CUDA）を提供する
- 公式最適化アーティファクトがローカルにある場合はそれを優先してロードする
  - WindowsはCUDA、macOSはMetalの最小経路を先に確立する
  - 対応不可の場合は明確に未対応として扱い、ready 一覧から除外できる

## 決定事項（設計合意）
- 「公式最適化アーティファクト」は、同一 publisher org（例: `openai`, `nvidia`）配下の別リポジトリに存在してよい。
- 取得元は許可リストで管理する（許可リスト外は無視）。
- 許可リスト初期値: `openai/*`, `nvidia/*`
- 登録形式は常に `format=safetensors` を維持し、公式最適化アーティファクトは実行キャッシュとして扱う。

## 主要な要明確化（実装前に決めること）
- Windows CUDA 実行の実装範囲（初期は最小機能で成立させる）。
- 公式GPU最適化アーティファクトの「自動利用 / 明示 opt-in」方針。
- プラグイン ABI の固定方針（バージョン更新ルール）。

## テスト方針（TDD）
- Contract: Router API（登録/一覧）と Node API（chat/completions）の契約を増やす
- Integration: gpt-oss-20b を `format=safetensors` で登録 → Node がロード → 生成成功、を最小経路で確認
- E2E: ダッシュボードからの登録 → チャット疎通（可能なら）

## ドキュメント
- README に「safetensorsを正本として登録する」「gpt-oss-20b の前提ファイル」「未対応時の挙動」を追記する。
