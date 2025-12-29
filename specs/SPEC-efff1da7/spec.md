# SPEC-efff1da7: Nemotron safetensors-cpp PoC

## 背景 / 問題
Nemotron 3 Nano 30B A3B(BF16) は llama.cpp の `convert_hf_to_gguf.py` ではテンソル名のマッピング不足で変換に失敗する。safetensors-cpp を使って safetensors を直接読み込み、テンソル構造と命名の実態を把握できる PoC が必要。

## 目的
本PoCは統合仕様 `SPEC-3fc2c1e4`（実行エンジン）の**調査枠**として位置付ける。

- safetensors-cpp を利用して Nemotron の safetensors を読み込めることを確認する
- テンソル名と型の概要を可視化し、MoE/experts 系テンソルの存在を確認する
- 変換可否の判断材料を得る

## ゴール
- `poc/nemotron-safetensors-cpp` に PoC 実装を追加
- PoC は safetensors を mmap で読み込み、テンソル数・dtype 別数・`experts` 系の件数を出力できる
- README にビルド/実行手順と注意点（巨大モデル/ダウンロード）を明記

## 非ゴール
- GGUF への完全変換実装
- llama.cpp 側への本実装変更
- Node/Router の本番機能追加

## ユーザーストーリー
- 開発者として、safetensors-cpp を使って Nemotron の safetensors を読み込み、テンソル命名や MoE 構造を把握したい。
- 開発者として、変換失敗の原因となるテンソル名が実際に存在することを確認したい。

## 受け入れ条件
- PoC が safetensors-cpp を使用して safetensors を読み込む
- 実行時に以下を出力する
  - 総テンソル数
  - dtype 別の件数
  - `experts` を含むテンソル名の件数
  - 既知の失敗テンソル名（`backbone.layers.1.mixer.experts.0.down_proj.weight`）の有無
- README にビルド・実行例がある
