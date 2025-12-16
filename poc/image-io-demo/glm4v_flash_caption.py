#!/usr/bin/env python3
"""
Image-to-Text PoC using zai-org/GLM-4.6V-Flash (transformers).

This PoC intentionally uses the Hugging Face distributed weights as-is.
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path


DEFAULT_MODEL = "zai-org/GLM-4.6V-Flash"


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
    parser.add_argument("--image", required=True, help="Image path (png/jpg) or URL")
    parser.add_argument("--prompt", default="describe this image")
    parser.add_argument("--out", default="")
    parser.add_argument("--device", default="auto", help="auto | cuda | mps (default: auto)")
    parser.add_argument("--require-gpu", action="store_true", help="Fail if cuda/mps is not available")
    parser.add_argument("--max-new-tokens", type=int, default=256)
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    if args.require_gpu and os.environ.get("PYTORCH_ENABLE_MPS_FALLBACK") == "1":
        print(
            "Error: PYTORCH_ENABLE_MPS_FALLBACK=1 is set. CPU fallback is not allowed when --require-gpu is used.",
            file=sys.stderr,
        )
        return 1

    try:
        import torch
        import torchvision  # required for BaseVideoProcessor (torchvision.transforms.v2)
        from torchvision.transforms.v2 import functional as _tvF  # noqa: F401
        from transformers import AutoProcessor, Glm46VForConditionalGeneration
    except Exception as e:
        print("Missing dependencies for GLM-4.6V PoC.", file=sys.stderr)
        print(
            "Create a venv and install: torch, torchvision, pillow, accelerate, and transformers (>=5.0.0rc0) including Glm46VForConditionalGeneration.",
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

    if args.verbose:
        from transformers.utils import logging as hf_logging

        hf_logging.set_verbosity_info()

    processor = AutoProcessor.from_pretrained(args.model)

    if device == "cuda":
        model = Glm46VForConditionalGeneration.from_pretrained(
            pretrained_model_name_or_path=args.model,
            torch_dtype="auto",
            device_map="auto",
        )
    elif device == "mps":
        model = Glm46VForConditionalGeneration.from_pretrained(
            pretrained_model_name_or_path=args.model,
            torch_dtype="auto",
            device_map=None,
        )
        model.to("mps")
    else:
        model = Glm46VForConditionalGeneration.from_pretrained(
            pretrained_model_name_or_path=args.model,
            torch_dtype="auto",
            device_map="cpu",
        )

    model.eval()

    messages = [
        {
            "role": "user",
            "content": [
                {"type": "image", "url": args.image},
                {"type": "text", "text": args.prompt},
            ],
        }
    ]

    inputs = processor.apply_chat_template(
        messages,
        tokenize=True,
        add_generation_prompt=True,
        return_dict=True,
        return_tensors="pt",
    ).to(model.device)
    inputs.pop("token_type_ids", None)

    with torch.inference_mode():
        generated_ids = model.generate(**inputs, max_new_tokens=args.max_new_tokens)

    output_text = processor.decode(
        generated_ids[0][inputs["input_ids"].shape[1]:],
        skip_special_tokens=False,
    )

    if args.out:
        out_path = Path(args.out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(output_text, encoding="utf-8")
        print(f"Caption written: {out_path}")
    else:
        print(output_text)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
