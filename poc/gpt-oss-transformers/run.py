#!/usr/bin/env python3
"""
Minimal text generation PoC for openai/gpt-oss-20b using Transformers.

This is *not* ONNX. It's intended to validate the chosen model can run locally
in a straightforward path on macOS (Apple Silicon) before attempting any
conversion / C++ integration.
"""

from __future__ import annotations

import argparse
import os
import sys
from typing import Any, Dict, List


DEFAULT_MODEL = "openai/gpt-oss-20b"


def _device_arg(value: str) -> str:
    v = value.strip().lower()
    if v not in ("", "auto", "cpu", "mps", "cuda"):
        raise argparse.ArgumentTypeError("device must be one of: auto|cpu|mps|cuda")
    return v or "auto"


def _build_messages(prompt: str) -> List[Dict[str, str]]:
    # Let transformers chat template apply harmony format for gpt-oss.
    return [{"role": "user", "content": prompt}]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--prompt", default="Explain quantum mechanics clearly and concisely.")
    parser.add_argument("--max-new-tokens", type=int, default=256)
    parser.add_argument("--device", type=_device_arg, default="auto")
    args = parser.parse_args()

    try:
        import torch
        from transformers import pipeline
    except Exception as e:
        print("Missing dependencies. Run: pip install -r requirements.txt", file=sys.stderr)
        print(f"Import error: {e}", file=sys.stderr)
        return 1

    model_id = args.model
    prompt = args.prompt.strip()
    if not prompt:
        print("Error: empty prompt", file=sys.stderr)
        return 1

    device_map: Any
    if args.device in ("", "auto"):
        device_map = "auto"
    elif args.device == "cpu":
        device_map = {"": "cpu"}
    elif args.device == "mps":
        # Some setups don't support accelerate device_map for MPS well; try explicit mapping.
        device_map = {"": "mps"}
    else:
        device_map = {"": args.device}

    # Prefer "auto" dtype to respect quantization configs when supported.
    pipe = pipeline(
        "text-generation",
        model=model_id,
        torch_dtype="auto",
        device_map=device_map,
    )

    messages = _build_messages(prompt)
    outputs = pipe(messages, max_new_tokens=args.max_new_tokens)

    # HF pipeline returns a list; gpt-oss returns generated_text as list of messages.
    last = outputs[0]["generated_text"][-1]
    print(last)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

