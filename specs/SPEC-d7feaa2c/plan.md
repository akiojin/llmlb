# SPEC-d7feaa2c: Plan

## 方針
- Node側にエンジン抽象化レイヤーを導入し、実行エンジンを差し替え可能にする
- 内蔵エンジンは **プラグイン形式（動的ロード）** とし、C ABI固定で互換性を担保する
- 既存 llama.cpp は plugin として提供する
- エンジン選択は「登録時に選択したアーティファクト（safetensors/GGUF）」と
  Hugging Face の `config.json` 等のモデル由来メタデータを正として判定する
- `metadata.json` のような llm-router 独自メタデータファイルは使用しない
- Nemotron向けの新エンジン（推論エンジン）の仕様/実装は別SPECで後日決定（TBD）

## 設計スコープ（Router/Nodeの責務）

### Router（登録・配布）
- モデル登録時に「どのアーティファクトを使うか」を確定し、Nodeが迷わない形で配布する
  - safetensors/GGUF が両方ある場合は登録時に必須選択（`format`）
  - 選択された形式に応じて、配布マニフェスト（必要ファイル一覧）を確定する
- Node同期用API（`/v0/models` と `/v0/models/registry/...`）は「登録時に確定したもの」をそのまま返す
- llm-router独自の `metadata.json` は生成・参照しない

### Node（取得・検証・実行）
- Routerのマニフェストに従い、必要ファイルをローカルに取得（または共有パスを直接参照）する
- ローカル配置されたモデルを、`config.json` 等のHF由来メタデータで検証し、実行エンジンを決定する
- エンジンが未対応のモデルは「利用可能モデル一覧」から除外できる
- EngineHost（Plugin Loader）が runtime/format に一致する plugin をロードする

## エンジン選択（判定ソース）
- 判定の正は以下のみ
  1) 登録時に選択されたアーティファクト形式（safetensors/GGUF）
  2) Hugging Faceスナップショットのメタデータ（`config.json` など）
- ローカルに両形式が同居する前提の自動フォールバックは行わない（登録時に選択しているため）

## safetensorsモデルの取り扱い（ロード前提）
- `config.json` と `tokenizer.json` は必須（不足は登録時または同期時にエラー）
- 重みは以下のいずれかを「primary」として扱う
  - `*.safetensors.index.json`（シャーディングあり）
  - `*.safetensors`（単一ファイルのみ許可）
- `index` が無いのに `*.safetensors` が複数ある場合は曖昧としてエラー（どれが正本か決められない）

## GGUFモデルの取り扱い（フォールバック用途）
- GGUFは「正本」ではなく実行キャッシュとして扱う
- 登録時にGGUFファイル（または選択ポリシー）を確定し、Nodeは `model.gguf` として実行する

## 実装概要（Node側抽象化レイヤー）

### 1) Node側抽象化
- `EngineHost`（Plugin Loader）でプラグインをロード
- `EngineRegistry` で runtime ID → plugin を解決
- `Engine` は C ABI で提供し、manifest/abi_version を検証する

### 2) ModelStorage拡張
- Routerが配布したローカルのモデル配置（= 登録時の選択結果を反映）と
  `config.json` 等のメタデータを読み取り、`ModelDescriptor` として返す
- `listAvailable()` は「選択されたアーティファクトがローカルに存在するか」と
  「対応エンジンがあるか」で有効モデルを列挙する

### 3) Router側最小対応
- Nodeが必要とする「登録時の選択情報」を永続化し、Nodeへ渡せるようにする
- `metadata.json` を生成・参照しない

## テスト
- ModelStorage: `format` / 必須メタデータの検証テスト
- Engine selection: 登録時選択と `config.json` に基づく判定テスト
- Plugin loader: manifest/ABIバージョン検証テスト

## 互換性・保留（TBD）

### chat_template（完全互換）
- Nemotronを含むHFモデルの `chat_template` はJinja互換が前提のため、完全互換の扱いが必要
- ただし Nemotron向けの推論エンジンは後回し（別SPEC）なので、本SPECでは「要求の明記」に留める
  - 推奨案: Router（Rust）でJinja互換レンダリングを実装し、Nodeには「最終プロンプト」を渡す
    - 理由: Node（C++）にJinja完全互換実装を持ち込むと依存・ビルドが重くなるため
    - 位置づけ: ルーティング前処理（プロンプト形成）と、推論（GPU実行）を分離する
  - 互換性の目標: Hugging Face Transformers の chat_template（Jinja2）と同等のレンダリング結果
    - `messages` / `add_generation_prompt` などの標準変数を提供
    - token系（`bos_token`/`eos_token` 等）は `tokenizer.json` 由来の値を用いる
    - `chat_template` が無い場合はデフォルトテンプレート（例: ChatML）へフォールバック
  - 生成結果のパース: テンプレート由来の制御トークン（例: ChatMLやchannel marker）を除去し、最終メッセージを抽出する

### Metal/DirectML（GPU実行）
- NodeはGPU必須（Apple Silicon/Metal、Windows/DirectMLを主対象）
- 具体的なカーネル実装・dtype戦略は別SPECで決める（CUDAは実験扱い）

## Nemotron GPU PoC（後回し / TBD）

### 目的
- safetensors（正本）を直接読み込み、Metal/DirectMLのGPUで「実推論できる」ことを実証する
  - ここでの「実推論」は、最低でも 1トークン以上の生成が再現できること

### 段階（PoCの定義）
1. **重みの直読**: safetensors-cppでmmapし、index/shard構成でも破綻なく参照できる
2. **メタデータ整合**: `config.json` と `tokenizer.json` が揃い、必要なハイパーパラメータが解釈できる
3. **CPU参照実装（最小）**: 代表的な演算（matmul/norm/attention/MLP）をCPUで通せる（小モデルでOK）
4. **GPU実装（最小）**: Metal/DirectMLで主要演算を実装し、同一入力でCPU参照と一致（または許容誤差内）
5. **E2E生成**: 実際のトークナイズ→生成→デトークナイズまで一連で動作

### 現状（本ブランチでの到達点）
- safetensors-cpp による「重み直読PoC（テンソル列挙・検証）」まで
- GPU推論PoCは Nemotron推論エンジンと一体のため、別SPECで後日実施
