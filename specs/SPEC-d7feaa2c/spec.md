# SPEC-d7feaa2c: Nodeエンジンローダー抽象化とNemotron直接ロード

## 背景 / 問題
Nodeは現在 llama.cpp (GGUF) を前提にしており、モデル形式の多様化（safetensors等）に対応できない。
Nemotron 3 Nano 30B A3B(BF16) はGGUF変換が失敗するため、直接ロード可能な新エンジンが必要。

## 目的
- Node側でエンジンローダーを抽象化し、複数エンジンを共存可能にする
- Nemotron向けの新エンジンを追加し、safetensorsを直接ロードできる状態にする
- エンジン選択は「登録時に選択したアーティファクト（safetensors/GGUF）」と
  Hugging Face の `config.json` 等のモデル由来メタデータを正として判定する

## ゴール
- Nodeにエンジン抽象化レイヤーが導入され、llama.cpp と Nemotron エンジンを選択できる
- `metadata.json` のような llm-router 独自メタデータファイルに依存せず、エンジン選択が実装される
- safetensors-cpp を用いた Nemotron モデルの直接ロード（mmap + validation）が可能になる

## 非ゴール
- Nemotron 推論の高速化・最適化
- Python依存の導入
- GGUF変換の改善

## ユーザーストーリー
- 開発者として、モデル形式が増えてもNodeのエンジン選択が壊れない構造にしたい
- 開発者として、Nemotronをsafetensorsから直接ロードできるようにしたい

## 受け入れ条件
- Nodeが登録時の選択（safetensors/GGUF）と `config.json` に従ってエンジンを選択する
- `metadata.json` に依存しない
- Nemotronエンジンが safetensors を mmap で読み込み、
  data_offsets の検証と主要テンソルの存在確認を行える
- エンジン判定結果は /v1/models の応答に影響し、未対応モデルは登録対象から除外できる
- Python依存は導入しない
