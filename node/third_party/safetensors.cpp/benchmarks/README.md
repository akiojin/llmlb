# Benchmarks

Performance comparison tools for safetensors.cpp vs HuggingFace transformers.

## Quick Start

### Prerequisites

For HuggingFace benchmark:

```bash
pip install transformers torch
```

For safetensors.cpp benchmark:

```bash
# Build the benchmark tool
cd node/third_party/safetensors.cpp
mkdir build && cd build
cmake .. -DSTCPP_METAL=ON  # or -DSTCPP_CUDA=ON for NVIDIA
make benchmark
```

### Running Benchmarks

#### 1. Run safetensors.cpp benchmark

```bash
./build/examples/benchmark /path/to/model.safetensors \
    --prompt-tokens 128 \
    --gen-tokens 128 \
    --iterations 10 \
    --output stcpp_results.json
```

#### 2. Run HuggingFace benchmark

```bash
python benchmarks/hf_benchmark.py TinyLlama/TinyLlama-1.1B-Chat-v1.0 \
    --prompt-tokens 128 \
    --gen-tokens 128 \
    --iterations 10 \
    --output hf_results.json
```

#### 3. Compare results

```bash
python benchmarks/compare.py stcpp_results.json hf_results.json
```

Or export comparison as Markdown:

```bash
python benchmarks/compare.py stcpp_results.json hf_results.json --output-md comparison.md
```

## Benchmark Options

### safetensors.cpp (`benchmark`)

| Option | Default | Description |
|--------|---------|-------------|
| `--prompt-tokens N` | 128 | Number of prompt tokens |
| `--gen-tokens N` | 128 | Number of tokens to generate |
| `--batch-size N` | 1 | Batch size |
| `--iterations N` | 10 | Number of benchmark iterations |
| `--gpu-layers N` | all | Layers to offload to GPU |
| `--warmup N` | 2 | Warmup iterations |
| `--output FILE` | - | Output JSON file |

### HuggingFace (`hf_benchmark.py`)

| Option | Default | Description |
|--------|---------|-------------|
| `--prompt-tokens N` | 128 | Number of prompt tokens |
| `--gen-tokens N` | 128 | Number of tokens to generate |
| `--batch-size N` | 1 | Batch size |
| `--iterations N` | 10 | Number of benchmark iterations |
| `--warmup N` | 2 | Warmup iterations |
| `--device DEVICE` | auto | Device (cuda/mps/cpu) |
| `--dtype DTYPE` | float16 | Data type |
| `--output FILE` | - | Output JSON file |

## Metrics

| Metric | Description |
|--------|-------------|
| Prompt Processing | Tokens/sec for prompt encoding |
| Token Generation | Tokens/sec for text generation |
| Time to First Token | Latency to first generated token |
| Total Time | End-to-end generation time |
| VRAM Usage | GPU memory used by model |

## Example Output

```text
═══════════════════════════════════════════════════════════════════════════════════════════
               BENCHMARK COMPARISON: safetensors.cpp vs HuggingFace transformers
═══════════════════════════════════════════════════════════════════════════════════════════

Configuration:
  Model (stcpp):       TinyLlama-1.1B-Chat-v1.0
  Model (HF):          TinyLlama/TinyLlama-1.1B-Chat-v1.0
  Prompt tokens:       128
  Generated tokens:    128
  Batch size:          1
  Iterations:          10

──────────────────────────────────────────────────────────────────────────────────────────
Metric                    safetensors.cpp      HuggingFace        Diff
──────────────────────────────────────────────────────────────────────────────────────────
Prompt Processing            1500.00 tokens/sec   1200.00 tokens/sec ✓      +25.0%
Token Generation              120.00 tokens/sec     80.00 tokens/sec ✓      +50.0%
Time to First Token            85.00 ms            106.67 ms          ✓      +25.5%
Total Time                   1150.00 ms           1706.67 ms          ✓      +48.4%
VRAM Usage                      2200 MB              2800 MB          ✓      +27.3%
──────────────────────────────────────────────────────────────────────────────────────────

Token Generation Speedup: 1.50x
→ safetensors.cpp is significantly faster
```

## Notes

1. **Fair Comparison**: Ensure both benchmarks use the same:
   - Model (same weights, converted if necessary)
   - Prompt token count
   - Generation token count
   - Number of iterations

2. **GPU Backend**: For best performance, ensure GPU acceleration is enabled:
   - safetensors.cpp: Built with `-DSTCPP_METAL=ON` or `-DSTCPP_CUDA=ON`
   - HuggingFace: Running on CUDA or MPS device

3. **Warmup**: Both tools include warmup iterations to ensure stable measurements.

4. **Variability**: Run multiple iterations and compare averages for reliable results.
