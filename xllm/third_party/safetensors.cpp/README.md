# safetensors.cpp

A C++ library for loading and running inference on safetensors format LLMs using ggml backend.

## Features

- Direct loading of HuggingFace safetensors models (no GGUF conversion needed)
- Multi-GPU support: Metal, CUDA, ROCm, Vulkan
- Streaming token generation
- Continuous batching
- LoRA/QLoRA adapter support
- Prompt caching
- Thread-safe C API (`stcpp_*` prefix)

## Supported Architectures

- gpt-oss-20b
- nemotron
- (more coming via plugin system)

## Requirements

- CMake 3.18+
- C++17 compiler
- GPU environment:
  - macOS: Apple Silicon + Metal
  - Windows/Linux: NVIDIA GPU + CUDA 11.8+
  - Linux: AMD GPU + ROCm 5.0+
  - Vulkan SDK (optional)

## Build

```bash
# Clone with submodules
git submodule update --init --recursive

# Build
mkdir build && cd build

# macOS (Metal)
cmake .. -DSTCPP_METAL=ON
cmake --build . --config Release

# Linux/Windows (CUDA)
cmake .. -DSTCPP_CUDA=ON
cmake --build . --config Release

# Linux (ROCm)
cmake .. -DSTCPP_ROCM=ON
cmake --build . --config Release
```

## Quick Start

### CLI

```bash
./build/examples/main \
    --model ./model \
    --prompt "Hello, world!" \
    --max-tokens 100
```

### C API

```c
#include "safetensors.h"

int main() {
    stcpp_init();

    stcpp_model* model = stcpp_model_load("./model", NULL, NULL);
    stcpp_context_params params = stcpp_context_default_params();
    stcpp_context* ctx = stcpp_context_new(model, params);

    char output[4096];
    stcpp_sampling_params sampling = stcpp_sampling_default_params();
    stcpp_generate(ctx, "Hello, ", sampling, 100, output, sizeof(output));
    printf("%s\n", output);

    stcpp_context_free(ctx);
    stcpp_model_free(model);
    stcpp_free();
    return 0;
}
```

## License

MIT License - see [LICENSE](LICENSE)

## Related Projects

- [ggml](https://github.com/ggml-org/ggml) - Tensor library for ML
- [llama.cpp](https://github.com/ggerganov/llama.cpp) - GGUF inference
- [stable-diffusion.cpp](https://github.com/leejet/stable-diffusion.cpp) - Image generation
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) - Speech recognition
