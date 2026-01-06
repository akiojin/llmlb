# Data Model: Windows CUDA runtime DLL (gpt-oss/nemotron)

## Entities

### CudaRuntimeDll
- name: gptoss_cuda.dll / nemotron_cuda.dll
- source: env override or model directory or system path

### CudaArtifact
- primary: model.cuda.bin
- fallback: cuda/model.bin
- (optional) model.bin
