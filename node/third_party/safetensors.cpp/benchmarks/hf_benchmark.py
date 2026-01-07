#!/usr/bin/env python3
"""
HuggingFace transformers benchmark script for comparison with safetensors.cpp

Usage:
    python hf_benchmark.py <model_path> [options]

Options:
    --prompt-tokens N    Number of prompt tokens (default: 128)
    --gen-tokens N       Number of tokens to generate (default: 128)
    --batch-size N       Batch size (default: 1)
    --iterations N       Number of iterations (default: 10)
    --warmup N           Warmup iterations (default: 2)
    --device DEVICE      Device to use (cuda/mps/cpu, default: auto)
    --dtype DTYPE        Data type (float16/bfloat16/float32, default: float16)
    --output FILE        Output JSON file for results

Example:
    python hf_benchmark.py TinyLlama/TinyLlama-1.1B-Chat-v1.0 --gen-tokens 128
"""

import argparse
import json
import sys
import time
from dataclasses import asdict, dataclass
from typing import Optional

try:
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer
except ImportError:
    print("Error: transformers and torch are required.")
    print("Install with: pip install transformers torch")
    sys.exit(1)


@dataclass
class BenchmarkConfig:
    """Benchmark configuration"""

    model_path: str
    prompt_tokens: int = 128
    gen_tokens: int = 128
    batch_size: int = 1
    iterations: int = 10
    warmup: int = 2
    device: str = "auto"
    dtype: str = "float16"


@dataclass
class BenchmarkResults:
    """Benchmark results"""

    model_path: str
    backend: str = "huggingface"
    prompt_tokens: int = 0
    gen_tokens: int = 0
    batch_size: int = 0
    iterations: int = 0
    prompt_tokens_per_sec: float = 0.0
    gen_tokens_per_sec: float = 0.0
    total_time_ms: float = 0.0
    first_token_ms: float = 0.0
    load_time_ms: float = 0.0
    vram_used_mb: int = 0
    device: str = ""
    dtype: str = ""


def get_device(requested: str) -> str:
    """Determine the best available device"""
    if requested != "auto":
        return requested

    if torch.cuda.is_available():
        return "cuda"
    elif hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
        return "mps"
    return "cpu"


def get_dtype(requested: str) -> torch.dtype:
    """Convert string dtype to torch dtype"""
    dtype_map = {
        "float16": torch.float16,
        "bfloat16": torch.bfloat16,
        "float32": torch.float32,
    }
    return dtype_map.get(requested, torch.float16)


def generate_prompt(tokenizer, approx_tokens: int) -> str:
    """Generate a prompt with approximately the given number of tokens"""
    base = "The following is a detailed analysis of machine learning:\n\n"
    filler = (
        "In the field of artificial intelligence, machine learning "
        "represents a significant paradigm shift in how we approach "
        "computational problem-solving. "
    )

    prompt = base
    while True:
        tokens = tokenizer.encode(prompt, add_special_tokens=False)
        if len(tokens) >= approx_tokens:
            break
        prompt += filler

    # Truncate to exact token count
    tokens = tokenizer.encode(prompt, add_special_tokens=False)[:approx_tokens]
    return tokenizer.decode(tokens)


def get_vram_usage() -> int:
    """Get current VRAM usage in MB"""
    if torch.cuda.is_available():
        return torch.cuda.memory_allocated() // (1024 * 1024)
    return 0


def run_benchmark(config: BenchmarkConfig) -> BenchmarkResults:
    """Run the benchmark"""
    device = get_device(config.device)
    dtype = get_dtype(config.dtype)

    print(f"HuggingFace transformers Benchmark")
    print(f"Device: {device}, dtype: {config.dtype}")
    print()

    # Load model and tokenizer
    print(f"Loading model: {config.model_path}")
    load_start = time.perf_counter()

    tokenizer = AutoTokenizer.from_pretrained(
        config.model_path, trust_remote_code=True
    )

    model = AutoModelForCausalLM.from_pretrained(
        config.model_path,
        torch_dtype=dtype,
        device_map=device if device != "cpu" else None,
        trust_remote_code=True,
    )

    if device != "cpu" and model.device.type == "cpu":
        model = model.to(device)

    load_end = time.perf_counter()
    load_time_ms = (load_end - load_start) * 1000
    print(f"Model loaded in {load_time_ms:.2f} ms")

    # Get VRAM usage after loading
    vram_mb = get_vram_usage()

    # Generate test prompt
    prompt = generate_prompt(tokenizer, config.prompt_tokens)
    inputs = tokenizer(prompt, return_tensors="pt").to(model.device)
    actual_prompt_tokens = inputs.input_ids.shape[1]

    print(f"Prompt tokens: {actual_prompt_tokens}")
    print()

    # Warmup
    print(f"Running {config.warmup} warmup iterations...")
    for _ in range(config.warmup):
        with torch.no_grad():
            model.generate(
                inputs.input_ids,
                max_new_tokens=config.gen_tokens,
                do_sample=False,
                pad_token_id=tokenizer.eos_token_id,
            )

    # Benchmark iterations
    print(f"Running {config.iterations} benchmark iterations...")

    prompt_times: list[float] = []
    gen_times: list[float] = []
    first_token_times: list[float] = []
    total_times: list[float] = []

    for i in range(config.iterations):
        # Clear cache
        if torch.cuda.is_available():
            torch.cuda.synchronize()

        iter_start = time.perf_counter()

        with torch.no_grad():
            # Generate one token to measure time to first token
            first_token_start = time.perf_counter()
            _ = model.generate(
                inputs.input_ids,
                max_new_tokens=1,
                do_sample=False,
                pad_token_id=tokenizer.eos_token_id,
            )
            if torch.cuda.is_available():
                torch.cuda.synchronize()
            first_token_end = time.perf_counter()
            first_token_ms = (first_token_end - first_token_start) * 1000

            # Full generation
            full_gen_start = time.perf_counter()
            outputs = model.generate(
                inputs.input_ids,
                max_new_tokens=config.gen_tokens,
                do_sample=False,
                pad_token_id=tokenizer.eos_token_id,
            )
            if torch.cuda.is_available():
                torch.cuda.synchronize()
            full_gen_end = time.perf_counter()

        iter_end = time.perf_counter()
        total_ms = (iter_end - iter_start) * 1000
        full_gen_ms = (full_gen_end - full_gen_start) * 1000

        # Estimate prompt processing time (time to first token minus actual first token gen)
        prompt_ms = first_token_ms * 0.9  # Most of first token time is prompt processing
        gen_ms = full_gen_ms - prompt_ms

        prompt_times.append(prompt_ms)
        gen_times.append(gen_ms)
        first_token_times.append(first_token_ms)
        total_times.append(total_ms)

        tokens_generated = outputs.shape[1] - inputs.input_ids.shape[1]
        print(f"  Iteration {i + 1}/{config.iterations}: {total_ms:.2f} ms ({tokens_generated} tokens)")

    # Calculate averages
    avg_prompt_ms = sum(prompt_times) / len(prompt_times)
    avg_gen_ms = sum(gen_times) / len(gen_times)
    avg_first_token_ms = sum(first_token_times) / len(first_token_times)
    avg_total_ms = sum(total_times) / len(total_times)

    results = BenchmarkResults(
        model_path=config.model_path,
        backend="huggingface",
        prompt_tokens=actual_prompt_tokens,
        gen_tokens=config.gen_tokens,
        batch_size=config.batch_size,
        iterations=config.iterations,
        prompt_tokens_per_sec=(actual_prompt_tokens * 1000.0) / avg_prompt_ms if avg_prompt_ms > 0 else 0,
        gen_tokens_per_sec=(config.gen_tokens * 1000.0) / avg_gen_ms if avg_gen_ms > 0 else 0,
        total_time_ms=avg_total_ms,
        first_token_ms=avg_first_token_ms,
        load_time_ms=load_time_ms,
        vram_used_mb=vram_mb,
        device=device,
        dtype=config.dtype,
    )

    return results


def print_results(results: BenchmarkResults) -> None:
    """Print benchmark results"""
    sep = "━" * 78
    print()
    print(sep)
    print("                        BENCHMARK RESULTS")
    print(sep)
    print()
    print(f"Model: {results.model_path}")
    print(f"Backend: {results.backend}")
    print(f"Device: {results.device}")
    print(f"Data type: {results.dtype}")
    print(f"Prompt tokens: {results.prompt_tokens}")
    print(f"Generated tokens: {results.gen_tokens}")
    print(f"Batch size: {results.batch_size}")
    print(f"Iterations: {results.iterations}")
    print()
    print(sep)
    print("                        PERFORMANCE")
    print(sep)
    print()
    print(f"Prompt processing:    {results.prompt_tokens_per_sec:8.2f} tokens/sec")
    print(f"Token generation:     {results.gen_tokens_per_sec:8.2f} tokens/sec")
    print(f"Time to first token:  {results.first_token_ms:8.2f} ms")
    print(f"Total time:           {results.total_time_ms:8.2f} ms")
    print(f"Model load time:      {results.load_time_ms:8.2f} ms")
    print()
    print(sep)
    print("                        MEMORY")
    print(sep)
    print()
    print(f"VRAM used:            {results.vram_used_mb:8d} MB")
    print()
    print(sep)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="HuggingFace transformers benchmark for comparison with safetensors.cpp"
    )
    parser.add_argument("model_path", help="Path or HuggingFace model ID")
    parser.add_argument(
        "--prompt-tokens", type=int, default=128, help="Number of prompt tokens"
    )
    parser.add_argument(
        "--gen-tokens", type=int, default=128, help="Number of tokens to generate"
    )
    parser.add_argument("--batch-size", type=int, default=1, help="Batch size")
    parser.add_argument(
        "--iterations", type=int, default=10, help="Number of iterations"
    )
    parser.add_argument("--warmup", type=int, default=2, help="Warmup iterations")
    parser.add_argument(
        "--device", default="auto", help="Device (cuda/mps/cpu, default: auto)"
    )
    parser.add_argument(
        "--dtype", default="float16", help="Data type (float16/bfloat16/float32)"
    )
    parser.add_argument("--output", help="Output JSON file for results")

    args = parser.parse_args()

    config = BenchmarkConfig(
        model_path=args.model_path,
        prompt_tokens=args.prompt_tokens,
        gen_tokens=args.gen_tokens,
        batch_size=args.batch_size,
        iterations=args.iterations,
        warmup=args.warmup,
        device=args.device,
        dtype=args.dtype,
    )

    results = run_benchmark(config)
    print_results(results)

    if args.output:
        with open(args.output, "w") as f:
            json.dump(asdict(results), f, indent=2)
        print(f"\nResults saved to: {args.output}")


if __name__ == "__main__":
    main()
