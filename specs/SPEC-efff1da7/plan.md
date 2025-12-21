# SPEC-efff1da7: Plan

## 方針
- safetensors-cpp の単一ヘッダ `safetensors.hh` を PoC 配下に同梱する
- mmap ロードで巨大ファイルのメモリ消費を抑える
- 解析はテンソル名/型/件数の集計に限定する

## 実装概要
- `poc/nemotron-safetensors-cpp/main.cpp` を追加
  - CLI: `nemotron_safetensors_poc <safetensors_file> [--limit N] [--match STR]`
  - safetensors-cpp の `load_from_mmap` を利用
  - dtype 集計、`experts` を含むテンソル件数、既知テンソルの存在チェック
- `poc/nemotron-safetensors-cpp/README.md` に手順を記載

## 成果物
- PoC ソースコード
- README
- safetensors-cpp ヘッダ（ライセンス表記を README に記載）

## 検証
- Nemotron の safetensors シャードを 1 つダウンロードして PoC を実行し、出力を確認する
