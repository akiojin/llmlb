# Quickstart: Windows CUDA runtime DLL (gpt-oss/nemotron)

## 配置
- DLL: gptoss_cuda.dll / nemotron_cuda.dll
- CUDAアーティファクト: model.cuda.bin もしくは cuda/model.bin
- DLL の管理ソースは `node/engines/gptoss/cuda/` と `node/engines/nemotron/cuda/` に配置する

## 環境変数 (任意)
- XLLM_GPTOSS_CUDA_LIB=/path/to/gptoss_cuda.dll
- XLLM_NEMOTRON_CUDA_LIB=/path/to/nemotron_cuda.dll

## 期待挙動
- DLLとCUDAアーティファクトが揃っていれば /v1/models が ready になる
- 不足時は明確なエラーになる
