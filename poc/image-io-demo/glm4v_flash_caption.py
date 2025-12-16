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


def _strip_thinking_tokens(text: str) -> str:
    """Remove thinking/reasoning tokens from GLM output.

    GLM models may include internal reasoning in special token blocks.
    This function strips common patterns like <think>...</think> and similar markers.
    """
    import re

    # Remove <think>...</think> blocks (greedy, handles multiline)
    text = re.sub(r"<think>.*?</think>", "", text, flags=re.DOTALL | re.IGNORECASE)

    # Remove <reasoning>...</reasoning> blocks
    text = re.sub(r"<reasoning>.*?</reasoning>", "", text, flags=re.DOTALL | re.IGNORECASE)

    # Remove common special tokens
    tokens_to_remove = [
        "<|endoftext|>",
        "<|end|>",
        "<|assistant|>",
        "<|user|>",
        "<|system|>",
        "</s>",
        "<s>",
    ]
    for token in tokens_to_remove:
        text = text.replace(token, "")

    # Strip leading/trailing whitespace and collapse multiple newlines
    text = text.strip()
    text = re.sub(r"\n{3,}", "\n\n", text)

    return text


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
    parser.add_argument("--quiet", "-q", action="store_true", help="Suppress progress logs")
    parser.add_argument("--json-output", action="store_true", help="Output caption in JSON format")
    parser.add_argument("--strip-thinking", action="store_true", help="Remove thinking tokens from output")
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
        from torch.nn import functional as torch_nn_F
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

    # Torch MPS backend does not support padding_mode="border" or mode="bicubic"
    # in grid_sample, which GLM uses for the visual embeddings.
    # Patch it here so PoC still runs on Apple Silicon.
    if torch.backends.mps.is_available():
        orig_grid_sample = torch_nn_F.grid_sample

        def _patched_grid_sample(*args, **kwargs):
            if kwargs.get("padding_mode") == "border":
                kwargs["padding_mode"] = "zeros"
            if kwargs.get("mode") == "bicubic":
                kwargs["mode"] = "bilinear"
            return orig_grid_sample(*args, **kwargs)

        torch_nn_F.grid_sample = _patched_grid_sample

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

    # Strip thinking tokens if requested
    if args.strip_thinking:
        output_text = _strip_thinking_tokens(output_text)

    # Format output
    if args.json_output:
        import json
        result = {"caption": output_text, "model": args.model, "image": args.image}
        output_str = json.dumps(result, ensure_ascii=False, indent=2)
    else:
        output_str = output_text

    if args.out:
        out_path = Path(args.out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(output_str, encoding="utf-8")
        if not args.quiet:
            print(f"Caption written: {out_path}")
    else:
        print(output_str)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
