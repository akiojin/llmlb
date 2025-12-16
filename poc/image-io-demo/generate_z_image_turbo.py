#!/usr/bin/env python3
"""
Text-to-Image PoC using Tongyi-MAI/Z-Image-Turbo (diffusers).

This PoC intentionally uses the Hugging Face distributed weights as-is (safetensors).
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path


DEFAULT_MODEL = "Tongyi-MAI/Z-Image-Turbo"


def _choose_device(requested: str, require_gpu: bool) -> str:
    requested = requested.strip().lower()
    if requested == "mpx":
        requested = "mps"
    if requested in ("", "auto"):
        import torch

        if torch.cuda.is_available():
            return "cuda"
        if torch.backends.mps.is_available():
            return "mps"
        if require_gpu:
            raise RuntimeError("No GPU backend available (expected cuda or mps)")
        return "cpu"

    if requested == "cpu":
        if require_gpu:
            raise RuntimeError("CPU is not allowed (require-gpu is set)")
        return "cpu"

    if requested == "cuda":
        import torch

        if not torch.cuda.is_available():
            raise RuntimeError("CUDA is not available")
        return "cuda"

    if requested == "mps":
        import torch

        if not torch.backends.mps.is_available():
            raise RuntimeError("MPS is not available")
        return "mps"

    raise RuntimeError(f"Invalid device: {requested} (expected auto|cuda|mps)")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--prompt", required=True)
    parser.add_argument("--out", default="z_image_out.png")
    parser.add_argument("--device", default="auto", help="auto | cuda | mps (default: auto)")
    parser.add_argument("--require-gpu", action="store_true", help="Fail if cuda/mps is not available")
    parser.add_argument("--height", type=int, default=512)
    parser.add_argument("--width", type=int, default=512)
    parser.add_argument("--steps", type=int, default=9, help="num_inference_steps (Turbo recommends 9 -> 8 forwards)")
    parser.add_argument("--guidance", type=float, default=0.0, help="Turbo recommends 0.0")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--dtype", default="auto", help="auto | float32 | float16 | bfloat16 (default: auto)")
    args = parser.parse_args()

    if args.require_gpu and os.environ.get("PYTORCH_ENABLE_MPS_FALLBACK") == "1":
        print(
            "Error: PYTORCH_ENABLE_MPS_FALLBACK=1 is set. CPU fallback is not allowed when --require-gpu is used.",
            file=sys.stderr,
        )
        return 1

    try:
        import torch
        from diffusers import ZImagePipeline
    except Exception as e:
        print("Missing dependencies for Z-Image PoC.", file=sys.stderr)
        print(
            "Create a venv and install: torch, pillow, accelerate, and diffusers (git) with ZImagePipeline support.",
            file=sys.stderr,
        )
        print(f"Import error: {e}", file=sys.stderr)
        return 1

    try:
        device = _choose_device(args.device, args.require_gpu)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

    if device == "cpu" and args.require_gpu:
        print("Error: CPU is not allowed (require-gpu is set)", file=sys.stderr)
        return 1

    if args.dtype == "auto":
        if device == "cuda":
            torch_dtype = torch.bfloat16 if torch.cuda.is_bf16_supported() else torch.float16
        elif device == "mps":
            torch_dtype = torch.float16
        else:
            torch_dtype = torch.float32
    else:
        dtype_map = {
            "float32": torch.float32,
            "float16": torch.float16,
            "bfloat16": torch.bfloat16,
        }
        torch_dtype = dtype_map.get(args.dtype.lower())
        if torch_dtype is None:
            print(f"Error: invalid dtype={args.dtype} (expected auto|float32|float16|bfloat16)", file=sys.stderr)
            return 1

    print(f"Using device: {device} (dtype={torch_dtype})")
    print(f"Loading pipeline: {args.model}")

    pipe = ZImagePipeline.from_pretrained(
        args.model,
        torch_dtype=torch_dtype,
        low_cpu_mem_usage=True,
    )
    pipe.to(device)

    generator = torch.Generator(device=device).manual_seed(args.seed)
    image = pipe(
        prompt=args.prompt,
        height=args.height,
        width=args.width,
        num_inference_steps=args.steps,
        guidance_scale=args.guidance,
        generator=generator,
    ).images[0]

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    image.save(out_path)
    print(f"PNG written: {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
