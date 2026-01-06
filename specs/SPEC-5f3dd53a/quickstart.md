# Quickstart: Windows CUDA runtime DLL (gpt-oss/nemotron)

## 配置
- DLL: gptoss_cuda.dll / nemotron_cuda.dll
- CUDAアーティファクト: model.cuda.bin もしくは cuda/model.bin
- DLL の管理ソースは `node/src/cuda/` に配置する

## 環境変数 (任意)
- LLM_NODE_GPTOSS_CUDA_LIB=/path/to/gptoss_cuda.dll
- LLM_NODE_NEMOTRON_CUDA_LIB=/path/to/nemotron_cuda.dll

## 期待挙動
- DLLとCUDAアーティファクトが揃っていれば /v1/models が ready になる
- 不足時は明確なエラーになる
