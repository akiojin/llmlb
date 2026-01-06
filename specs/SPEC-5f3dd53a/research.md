# Research: Windows CUDA runtime DLL (gpt-oss/nemotron)

## 調査対象
- gptoss_* C API のCUDA版提供有無
- DLLの配布/ビルド手順
- CUDAアーティファクト命名と配置慣例

## 初期メモ
- 現状の仕様ではCUDA DLLはTBD扱い
- DirectMLは凍結

## 調査結果 (2026-01-06)
- `node/third_party/openai-gpt-oss` には **Metal 向けの gptoss_* C API 実装のみ** が含まれており、CUDA向けのC API / DLL 実装は見当たらない。
- `poc/gpt-oss-cuda` は **llama.cpp + GGUF** を用いた PoC であり、safetensors 直接実行や `gptoss_*` API の DLL 生成には繋がらない。
- `poc/nemotron-cuda-cpp` は safetensors + CUDA 直ロードの PoC 実装だが、**DLL化や gptoss_* API 互換** には未対応。
- PoC は参考用であり、CUDA DLL の正は `node/engines/gptoss/cuda/` と `node/engines/nemotron/cuda/` に置く方針。

## 追加の確認事項
- gptoss CUDA の C API / DLL をどこから調達するか（社内配布 or 新規実装）。
- nemotron CUDA PoC を DLL 化し、`gptoss_*` API 互換レイヤーとして整理するか。
