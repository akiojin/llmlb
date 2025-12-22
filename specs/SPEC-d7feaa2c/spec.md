# SPEC-d7feaa2c: Nodeエンジンローダー抽象化とNemotron直接ロード

## 背景 / 問題
Nodeは現在 llama.cpp (GGUF) を前提にしており、モデル形式の多様化（safetensors等）に対応できない。
Nemotron 3 Nano 30B A3B(BF16) はGGUF変換が失敗するため、safetensorsを直接実行できる新エンジンが必要。
ただし、Nemotron向けの新エンジン（推論エンジン）の仕様と実装は後回しにし、後で決める（TBD）。

## 目的
- Node側でエンジンローダーを抽象化し、複数エンジンを共存可能にする
- エンジン選択は「登録時に選択したアーティファクト（safetensors/GGUF）」と
  Hugging Face の `config.json` 等のモデル由来メタデータを正として判定する

## ゴール
- Nodeにエンジン抽象化レイヤーが導入され、llama.cpp 等の複数エンジンを選択できる
- `metadata.json` のような llm-router 独自メタデータファイルに依存せず、エンジン選択が実装される

## 非ゴール
- Nemotron向けの新エンジン（推論エンジン）の仕様策定・実装（後回し / TBDとして別途扱う）
- Nemotron 推論の高速化・最適化
- Python依存の導入
- GGUF変換の改善

## ユーザーストーリー
- 開発者として、モデル形式が増えてもNodeのエンジン選択が壊れない構造にしたい

## 保留事項（TBD）
- Nemotron向けの新エンジン（推論エンジン）は別SPECとして後日仕様化する（後回し / TBD。Metal/CUDA対応方針、chat_template互換、dtype戦略など）。

## 受け入れ条件
- Nodeが登録時の選択（safetensors/GGUF）と `config.json` に従ってエンジンを選択する
- `metadata.json` に依存しない
- エンジン判定結果は /v1/models の応答に影響し、未対応モデルは登録対象から除外できる
- Python依存は導入しない
