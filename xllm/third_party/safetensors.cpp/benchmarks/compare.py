#!/usr/bin/env python3
"""
Benchmark comparison tool for safetensors.cpp vs HuggingFace transformers

Usage:
    python compare.py <stcpp_results.json> <hf_results.json>

This script compares benchmark results from safetensors.cpp and HuggingFace
transformers, generating a comparison report.
"""

import argparse
import json
import sys
from dataclasses import dataclass
from typing import Any


@dataclass
class ComparisonMetric:
    """Single comparison metric"""

    name: str
    stcpp_value: float
    hf_value: float
    unit: str
    higher_is_better: bool

    @property
    def speedup(self) -> float:
        """Calculate speedup ratio (positive = stcpp is better)"""
        if self.hf_value == 0:
            return 0.0
        if self.higher_is_better:
            return self.stcpp_value / self.hf_value
        return self.hf_value / self.stcpp_value

    @property
    def percentage_diff(self) -> float:
        """Percentage difference (positive = stcpp is better)"""
        return (self.speedup - 1.0) * 100


def load_results(path: str) -> dict[str, Any]:
    """Load benchmark results from JSON file"""
    with open(path) as f:
        return json.load(f)


def format_value(value: float, unit: str) -> str:
    """Format a value with its unit"""
    if unit == "tokens/sec":
        return f"{value:.2f} {unit}"
    elif unit == "ms":
        return f"{value:.2f} {unit}"
    elif unit == "MB":
        return f"{int(value)} {unit}"
    return f"{value} {unit}"


def format_speedup(speedup: float, higher_is_better: bool) -> str:
    """Format speedup ratio with indicator"""
    if speedup > 1.0:
        indicator = "+" if higher_is_better else "-"
        return f"{indicator}{((speedup - 1) * 100):.1f}%"
    elif speedup < 1.0:
        indicator = "-" if higher_is_better else "+"
        return f"{indicator}{((1 - speedup) * 100):.1f}%"
    return "same"


def compare_results(
    stcpp_results: dict[str, Any], hf_results: dict[str, Any]
) -> list[ComparisonMetric]:
    """Compare two benchmark result sets"""
    metrics: list[ComparisonMetric] = []

    # Performance metrics (higher is better)
    metrics.append(
        ComparisonMetric(
            name="Prompt Processing",
            stcpp_value=stcpp_results.get("prompt_tokens_per_sec", 0),
            hf_value=hf_results.get("prompt_tokens_per_sec", 0),
            unit="tokens/sec",
            higher_is_better=True,
        )
    )

    metrics.append(
        ComparisonMetric(
            name="Token Generation",
            stcpp_value=stcpp_results.get("gen_tokens_per_sec", 0),
            hf_value=hf_results.get("gen_tokens_per_sec", 0),
            unit="tokens/sec",
            higher_is_better=True,
        )
    )

    # Latency metrics (lower is better)
    metrics.append(
        ComparisonMetric(
            name="Time to First Token",
            stcpp_value=stcpp_results.get("first_token_ms", 0),
            hf_value=hf_results.get("first_token_ms", 0),
            unit="ms",
            higher_is_better=False,
        )
    )

    metrics.append(
        ComparisonMetric(
            name="Total Time",
            stcpp_value=stcpp_results.get("total_time_ms", 0),
            hf_value=hf_results.get("total_time_ms", 0),
            unit="ms",
            higher_is_better=False,
        )
    )

    # Memory metrics (lower is better)
    metrics.append(
        ComparisonMetric(
            name="VRAM Usage",
            stcpp_value=stcpp_results.get("vram_used_mb", 0),
            hf_value=hf_results.get("vram_used_mb", 0),
            unit="MB",
            higher_is_better=False,
        )
    )

    return metrics


def print_comparison(
    stcpp_results: dict[str, Any],
    hf_results: dict[str, Any],
    metrics: list[ComparisonMetric],
) -> None:
    """Print comparison report"""
    sep = "═" * 90
    line = "─" * 90

    print()
    print(sep)
    print("               BENCHMARK COMPARISON: safetensors.cpp vs HuggingFace transformers")
    print(sep)
    print()

    # Model info
    print("Configuration:")
    print(f"  Model (stcpp):       {stcpp_results.get('model_path', 'N/A')}")
    print(f"  Model (HF):          {hf_results.get('model_path', 'N/A')}")
    print(f"  Prompt tokens:       {stcpp_results.get('prompt_tokens', 'N/A')}")
    print(f"  Generated tokens:    {stcpp_results.get('gen_tokens', 'N/A')}")
    print(f"  Batch size:          {stcpp_results.get('batch_size', 'N/A')}")
    print(f"  Iterations:          {stcpp_results.get('iterations', 'N/A')}")
    print()

    # Device info
    print("Environment:")
    print(f"  stcpp device:        {stcpp_results.get('device', 'N/A')}")
    print(f"  HF device:           {hf_results.get('device', 'N/A')}")
    print(f"  HF dtype:            {hf_results.get('dtype', 'N/A')}")
    print()

    print(line)
    print(f"{'Metric':<25} {'safetensors.cpp':>20} {'HuggingFace':>20} {'Diff':>12}")
    print(line)

    for metric in metrics:
        stcpp_str = format_value(metric.stcpp_value, metric.unit)
        hf_str = format_value(metric.hf_value, metric.unit)
        diff_str = format_speedup(metric.speedup, metric.higher_is_better)

        # Color indicator (in terminal)
        if metric.speedup > 1.0:
            indicator = "✓" if metric.higher_is_better else "✗"
        elif metric.speedup < 1.0:
            indicator = "✗" if metric.higher_is_better else "✓"
        else:
            indicator = "="

        print(f"{metric.name:<25} {stcpp_str:>20} {hf_str:>20} {indicator} {diff_str:>10}")

    print(line)
    print()

    # Summary
    better_count = sum(1 for m in metrics if m.speedup > 1.0)
    worse_count = sum(1 for m in metrics if m.speedup < 1.0)
    same_count = sum(1 for m in metrics if m.speedup == 1.0)

    print("Summary:")
    print(f"  safetensors.cpp better: {better_count} metrics")
    print(f"  HuggingFace better:     {worse_count} metrics")
    print(f"  Same:                   {same_count} metrics")
    print()

    # Calculate overall performance score
    gen_speedup = metrics[1].speedup  # Token generation is most important
    print(f"Token Generation Speedup: {gen_speedup:.2f}x")

    if gen_speedup > 1.5:
        print("→ safetensors.cpp is significantly faster")
    elif gen_speedup > 1.1:
        print("→ safetensors.cpp is faster")
    elif gen_speedup > 0.9:
        print("→ Performance is comparable")
    elif gen_speedup > 0.67:
        print("→ HuggingFace is faster")
    else:
        print("→ HuggingFace is significantly faster")

    print()
    print(sep)


def export_markdown(
    stcpp_results: dict[str, Any],
    hf_results: dict[str, Any],
    metrics: list[ComparisonMetric],
    output_path: str,
) -> None:
    """Export comparison as Markdown table"""
    with open(output_path, "w") as f:
        f.write("# Benchmark Comparison: safetensors.cpp vs HuggingFace transformers\n\n")

        f.write("## Configuration\n\n")
        f.write(f"- **Model**: {stcpp_results.get('model_path', 'N/A')}\n")
        f.write(f"- **Prompt tokens**: {stcpp_results.get('prompt_tokens', 'N/A')}\n")
        f.write(f"- **Generated tokens**: {stcpp_results.get('gen_tokens', 'N/A')}\n")
        f.write(f"- **Batch size**: {stcpp_results.get('batch_size', 'N/A')}\n")
        f.write(f"- **Iterations**: {stcpp_results.get('iterations', 'N/A')}\n\n")

        f.write("## Results\n\n")
        f.write("| Metric | safetensors.cpp | HuggingFace | Diff |\n")
        f.write("|--------|-----------------|-------------|------|\n")

        for metric in metrics:
            stcpp_str = format_value(metric.stcpp_value, metric.unit)
            hf_str = format_value(metric.hf_value, metric.unit)
            diff_str = format_speedup(metric.speedup, metric.higher_is_better)
            f.write(f"| {metric.name} | {stcpp_str} | {hf_str} | {diff_str} |\n")

        f.write("\n## Summary\n\n")
        gen_speedup = metrics[1].speedup
        f.write(f"**Token Generation Speedup**: {gen_speedup:.2f}x\n")

    print(f"Markdown report saved to: {output_path}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Compare safetensors.cpp and HuggingFace benchmark results"
    )
    parser.add_argument("stcpp_results", help="safetensors.cpp results JSON file")
    parser.add_argument("hf_results", help="HuggingFace results JSON file")
    parser.add_argument("--output-md", help="Output Markdown report file")

    args = parser.parse_args()

    try:
        stcpp_results = load_results(args.stcpp_results)
        hf_results = load_results(args.hf_results)
    except FileNotFoundError as e:
        print(f"Error: {e}")
        sys.exit(1)
    except json.JSONDecodeError as e:
        print(f"Error parsing JSON: {e}")
        sys.exit(1)

    metrics = compare_results(stcpp_results, hf_results)
    print_comparison(stcpp_results, hf_results, metrics)

    if args.output_md:
        export_markdown(stcpp_results, hf_results, metrics, args.output_md)


if __name__ == "__main__":
    main()
