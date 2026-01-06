# 繧ｯ繧､繝・け繧ｹ繧ｿ繝ｼ繝・ Node繧ｨ繝ｳ繧ｸ繝ｳ繝ｭ繝ｼ繝繝ｼ謚ｽ雎｡蛹・
## 蜑肴署譚｡莉ｶ

| 鬆・岼 | 隕∽ｻｶ |
|------|------|
| OS | Windows (CUDA) |
| GPU | NVIDIA CUDA-capable GPU |
| 繝励Λ繧ｰ繧､繝ｳ | `engines/` 驟堺ｸ九↓驟咲ｽｮ貂医∩ |

## 蝓ｺ譛ｬ逧・↑菴ｿ逕ｨ萓・
### 繝励Λ繧ｰ繧､繝ｳ繝・ぅ繝ｬ繧ｯ繝医Μ讒区・

```bash
# macOS (Metal) 縺ｮ蝣ｴ蜷・ls engines/llama_cpp/metal/

# 蜃ｺ蜉帑ｾ・
# manifest.json
# libllama_engine.dylib
```

### manifest.json 縺ｮ遒ｺ隱・
```bash
cat engines/llama_cpp/metal/manifest.json
```

```json
{
  "id": "llama_cpp",
  "version": "1.0.0",
  "abi_version": "1",
  "gpu_backend": "metal",
  "architectures": ["llama", "mistral", "gemma", "phi"],
  "formats": ["gguf"],
  "binary": "libllama_engine.dylib"
}
```

### 繝弱・繝峨・襍ｷ蜍・
```bash
# Metal迺ｰ蠅・〒襍ｷ蜍・./llm-node --engines-dir ./engines

# 繧ｫ繧ｹ繧ｿ繝VRAM荳企剞繧呈欠螳・./llm-node --engines-dir ./engines --vram-limit 8G
```

### 繝励Λ繧ｰ繧､繝ｳ荳隕ｧ縺ｮ遒ｺ隱・
```bash
# Node API邨檎罰縺ｧ繝ｭ繝ｼ繝画ｸ医∩繧ｨ繝ｳ繧ｸ繝ｳ繧堤｢ｺ隱・curl http://localhost:3000/api/engines

# 繝ｬ繧ｹ繝昴Φ繧ｹ萓・
{
  "engines": [
    {
      "id": "llama_cpp",
      "version": "1.0.0",
      "gpu_backend": "metal",
      "architectures": ["llama", "mistral", "gemma", "phi"],
      "status": "loaded"
    }
  ]
}
```

### 繝｢繝・Ν縺ｮ繝ｭ繝ｼ繝・
```bash
# 謖・ｮ壹い繝ｼ繧ｭ繝・け繝√Ε縺ｮ繝｢繝・Ν繧偵Ο繝ｼ繝・curl -X POST http://localhost:3000/api/models/load \
  -H "Content-Type: application/json" \
  -d '{
    "model_id": "llama-3.2-1b",
    "format": "gguf"
  }'
```

### 謗ｨ隲悶・螳溯｡・
```bash
# 繧ｹ繝医Μ繝ｼ繝溘Φ繧ｰ逕滓・
curl -X POST http://localhost:3000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3.2-1b",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": true
  }'
```

## 繝励Λ繧ｰ繧､繝ｳ髢狗匱

### 譛蟆城剞縺ｮ繝励Λ繧ｰ繧､繝ｳ螳溯｣・
```c
// my_engine.c
#include "engine_api.h"

static EngineInfo info = {
    .abi_version = ENGINE_ABI_VERSION,
    .engine_id = "my_engine",
    .version = "1.0.0"
};

ENGINE_API EngineInfo* engine_get_info(void) {
    return &info;
}

ENGINE_API int engine_init(EngineConfig* config) {
    // 蛻晄悄蛹門・逅・    return ERR_OK;
}

ENGINE_API int engine_load_model(const char* model_path) {
    // 繝｢繝・Ν繝ｭ繝ｼ繝牙・逅・    return ERR_OK;
}

ENGINE_API int engine_generate(
    const char* prompt,
    TokenResult** results
) {
    // 謗ｨ隲門・逅・    return ERR_OK;
}

ENGINE_API void engine_shutdown(void) {
    // 繧ｯ繝ｪ繝ｼ繝ｳ繧｢繝・・蜃ｦ逅・}
```

### 繝薙Ν繝会ｼ・acOS・・
```bash
clang -shared -fPIC -o libmy_engine.dylib my_engine.c
```

### manifest.json 縺ｮ菴懈・

```json
{
  "id": "my_engine",
  "version": "1.0.0",
  "abi_version": "1",
  "gpu_backend": "metal",
  "architectures": ["custom"],
  "formats": ["safetensors"],
  "binary": "libmy_engine.dylib"
}
```

### 繝励Λ繧ｰ繧､繝ｳ縺ｮ驟咲ｽｮ

```bash
mkdir -p engines/my_engine/metal
mv libmy_engine.dylib engines/my_engine/metal/
mv manifest.json engines/my_engine/metal/
```

## 繧ｨ繝ｩ繝ｼ繝上Φ繝峨Μ繝ｳ繧ｰ

### ABI荳堺ｸ閾ｴ

```bash
# 繝励Λ繧ｰ繧､繝ｳ縺ｮABI繝舌・繧ｸ繝ｧ繝ｳ縺後・繧ｹ繝医→荳堺ｸ閾ｴ
{
  "error": {
    "message": "ABI version mismatch: expected 1, got 2",
    "type": "plugin_error",
    "code": "abi_mismatch"
  }
}
```

### 繧｢繝ｼ繧ｭ繝・け繝√Ε荳堺ｸ閾ｴ

```bash
# 繝｢繝・Ν繧｢繝ｼ繧ｭ繝・け繝√Ε縺後・繝ｩ繧ｰ繧､繝ｳ縺ｧ譛ｪ蟇ｾ蠢・{
  "error": {
    "message": "Architecture 'nemotron' not supported by plugin 'llama_cpp'",
    "type": "unsupported_error",
    "code": "architecture_mismatch"
  }
}
```

### VRAM荳崎ｶｳ

```bash
# 繝｢繝・Ν繝ｭ繝ｼ繝画凾縺ｮVRAM荳崎ｶｳ
{
  "error": {
    "message": "Insufficient VRAM: required 16GB, available 8GB",
    "type": "resource_error",
    "code": "oom_vram"
  }
}
```

## 蛻ｶ髯蝉ｺ矩・
| 鬆・岼 | 蛻ｶ髯・|
|------|------|
| ABI莠呈鋤 | 蜷御ｸABI繝舌・繧ｸ繝ｧ繝ｳ縺ｮ縺ｿ |
| GPU蠢・・| CPU繝輔か繝ｼ繝ｫ繝舌ャ繧ｯ髱槫ｯｾ蠢・|
| 繝励Λ繧ｰ繧､繝ｳ遶ｶ蜷・| 蜷御ｸID縺ｯ蜈育捩蜆ｪ蜈・|
| 繝阪ャ繝医Ρ繝ｼ繧ｯ | 繝励Λ繧ｰ繧､繝ｳ縺九ｉ縺ｮ螟夜Κ騾壻ｿ｡遖∵ｭ｢ |
| 繧ｵ繝ｳ繝峨・繝・け繧ｹ | 縺ｪ縺暦ｼ井ｿ｡鬆ｼ蜑肴署・・|

## 險ｭ螳壹が繝励す繝ｧ繝ｳ

### 迺ｰ蠅・､画焚

```bash
# 繝励Λ繧ｰ繧､繝ｳ繝・ぅ繝ｬ繧ｯ繝医Μ
export LLM_NODE_ENGINES_DIR=/custom/path/engines

# VRAM菴ｿ逕ｨ荳企剞
export LLM_NODE_VRAM_LIMIT=8589934592  # 8GB in bytes

# 繝ｪ繧ｽ繝ｼ繧ｹ逶｣隕夜俣髫・export LLM_NODE_MONITOR_INTERVAL_MS=1000
```

### 繧ｳ繝槭Φ繝峨Λ繧､繝ｳ繧ｪ繝励す繝ｧ繝ｳ

```bash
llm-node \
  --engines-dir ./engines \
  --vram-limit 8G \
  --monitor-interval 1000
```

## 繝医Λ繝悶Ν繧ｷ繝･繝ｼ繝・ぅ繝ｳ繧ｰ

### 繝励Λ繧ｰ繧､繝ｳ縺梧､懷・縺輔ｌ縺ｪ縺・
```bash
# 繝・ぅ繝ｬ繧ｯ繝医Μ讒区・繧堤｢ｺ隱・ls -R engines/

# manifest.json縺ｮ讒区枚繧堤｢ｺ隱・cat engines/llama_cpp/metal/manifest.json | jq .
```

### ABI繧ｨ繝ｩ繝ｼ

```bash
# 繝帙せ繝医・ABI繝舌・繧ｸ繝ｧ繝ｳ繧堤｢ｺ隱・./llm-node --version

# 繝励Λ繧ｰ繧､繝ｳ縺ｮABI繝舌・繧ｸ繝ｧ繝ｳ繧堤｢ｺ隱・cat engines/llama_cpp/metal/manifest.json | jq .abi_version
```

### GPU讀懷・螟ｱ謨・
```bash
# macOS: Metal蟇ｾ蠢懊ｒ遒ｺ隱・system_profiler SPDisplaysDataType

# Windows: DirectX 12蟇ｾ蠢懊ｒ遒ｺ隱・dxdiag
```

