# SPEC-2c0e5a9b: Plan

## 方釁E
- gpt-oss 用 runtime を�Eラグインとして追加し、GPU 実行！Eetal/CUDA�E�を提供すめE- エンジン選択�E既存�E抽象化！ESPEC-d7feaa2c`�E�を利用し、登録時�E `format` と `config.json` 等�E HF 由来メタチE�Eタに従う、E- gpt-oss 実行エンジンは **プラグイン形式（動皁E��ード！E* で提供する、E- `chat_template` の解釈�E C++ Node に寁E��ず、Router 側で Jinja 互換レンダリングを行い、Node には最終�Eロンプト�E�テキスト）を渡す方針を前提とする、E- Node は Python 依存なしで動作する（忁E��）、E- Nemotron 推論エンジンは本件では扱わなぁE��別SPECで後日�E�、E- 実行�E優先頁E��E
  - 公式�EGPU最適化アーチE��ファクト（バチE��エンド依存、許可リスト対象�E�E  - safetensors�E�正本�E�E- 対応OS/GPU: macOS=Metal、Windows=CUDA、Linuxは非対応、EirectMLは実験扱ぁE��E
## 対象モチE��とアーチE��ファクト（前提！E- 対象: `openai/gpt-oss-20b`
- 前提�E�EFスナップショチE���E�E
  - `config.json`, `tokenizer.json`�E�忁E��！E  - `model.safetensors.index.json` + `model-*.safetensors`�E�シャーチE��ング�E�E  - `chat_template.jinja`�E�任意！E- 備老E モチE��によっては公式�EGPU最適化アーチE��ファクト（侁E `metal/model.bin`�E�が提供される場合がある、E  - safetensors は常に正本として保持する、E- 実行�E「バチE��エンドに一致する公式最適化アーチE��ファクトが利用可能なら優先、無ければ safetensors」を基本とする�E�登録形式�E変えなぁE��、E
## 実裁E��コープ（設計！E
### Router�E�登録・配币E��E- `format=safetensors` 登録晁E
  - 忁E��メタチE�Eタ検証�E�Econfig.json`, `tokenizer.json`�E�E  - index/shards の整合検証�E�欠損があれば失敗！E- Node が忁E��とするファイル一覧�E��Eニフェスト）を確宁E- 公式�EGPU最適化アーチE��ファクトが利用可能な場合�E**実行キャチE��ュとして**マニフェストへ含める�E�登録形式�E変えなぁE��E  - 追加アーチE��ファクト�E対応モチE��定義�E�Eupported_models.json の artifacts�E�で持E��すめE- ルーターは事前キャチE��ュ前提を廁E��し、E*マニフェスト�E提示のみ**を担当する（取得�ENode主導！E- `chat_template` が無ぁE��合�EチE��ォルトテンプレートを提侁E
### Node�E�取得�E検証・実行！E- ModelStorage:
  - gpt-oss めE`config.json` から検�Eし、対忁Eruntime を決定できる
  - safetensors�E�Endex + shards�E�を 1 モチE��として検証できる
- Engine:
  - gpt-oss 用 runtime を�Eラグインとして追加し、GPU 実行！Eetal/CUDA�E�を提供すめE- 公式最適化アーチE��ファクトがローカルにある場合�Eそれを優先してロードすめE  - WindowsはCUDA、macOSはMetalの最小経路を�Eに確立すめE  - 対応不可の場合�E明確に未対応として扱ぁE��ready 一覧から除外できる

## 決定事頁E��設計合意！E- 「�E式最適化アーチE��ファクト」�E、同一 publisher org�E�侁E `openai`, `nvidia`�E��E下�E別リポジトリに存在してよい、E- 取得�Eは許可リストで管琁E��る（許可リスト外�E無視）、E- 許可リスト�E期値: `openai/*`, `nvidia/*`
- 登録形式�E常に `format=safetensors` を維持し、�E式最適化アーチE��ファクト�E実行キャチE��ュとして扱ぁE��E
## 主要な要�E確化（実裁E��に決めること�E�E- Windows CUDA 実行�E実裁E��E���E��E期�E最小機�Eで成立させる�E�、E- 公式GPU最適化アーチE��ファクト�E「�E動利用 / 明示 opt-in」方針、E- プラグイン ABI の固定方針（バージョン更新ルール�E�、E
## チE��ト方針！EDD�E�E- Contract: Router API�E�登録/一覧�E�と Node API�E�Ehat/completions�E��E契紁E��増やぁE- Integration: gpt-oss-20b めE`format=safetensors` で登録 ↁENode がローチEↁE生�E成功、を最小経路で確誁E- E2E: ダチE��ュボ�Eドから�E登録 ↁEチャチE��疎通（可能なら！E
## ドキュメンチE- README に「safetensorsを正本として登録する」「gpt-oss-20b の前提ファイル」「未対応時の挙動」を追記する、E
