#!/usr/bin/env python3
"""
VibeVoice-Realtime-0.5B minimal TTS PoC (PyTorch).

This uses the official `vibevoice` python package and downloads a default
voice prompt (.pt) from the VibeVoice GitHub repo on first run.

Notes:
- VibeVoice-Realtime is not supported by vanilla Transformers auto classes.
- The model is intended for English; other languages may produce odd outputs.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path
from typing import Optional
from urllib.request import urlopen, urlretrieve


DEFAULT_MODEL = "microsoft/VibeVoice-Realtime-0.5B"
GITHUB_VOICES_API = (
    "https://api.github.com/repos/microsoft/VibeVoice/contents/demo/voices/streaming_model?ref=main"
)
GITHUB_VOICES_RAW_BASE = (
    "https://raw.githubusercontent.com/microsoft/VibeVoice/main/demo/voices/streaming_model"
)


def _default_cache_dir() -> Path:
    env = os.environ.get("VIBEVOICE_CACHE_DIR", "").strip()
    if env:
        return Path(env)
    home = Path(os.environ.get("HOME", "."))
    return home / ".cache" / "llm-router" / "vibevoice"


def _fetch_json(url: str) -> object:
    with urlopen(url, timeout=30) as resp:
        return json.load(resp)


def _resolve_voice_preset(name_or_path: str, cache_dir: Path) -> Path:
    p = Path(name_or_path)
    if p.exists():
        return p

    voice_name = name_or_path
    if not voice_name.endswith(".pt"):
        voice_name += ".pt"

    voices_dir = cache_dir / "voices"
    voices_dir.mkdir(parents=True, exist_ok=True)
    dest = voices_dir / voice_name
    if dest.exists():
        return dest

    url = f"{GITHUB_VOICES_RAW_BASE}/{voice_name}"
    print(f"Downloading voice prompt: {url} -> {dest}")
    urlretrieve(url, dest)  # nosec - PoC: trusted upstream location
    return dest


def _pick_default_voice_name() -> str:
    data = _fetch_json(GITHUB_VOICES_API)
    if not isinstance(data, list):
        raise RuntimeError("Unexpected GitHub API response for voices list")

    # Prefer English voices, otherwise just take the first.
    candidates = [item.get("name") for item in data if isinstance(item, dict)]
    candidates = [c for c in candidates if isinstance(c, str) and c.endswith(".pt")]
    if not candidates:
        raise RuntimeError("No voice prompt presets found in GitHub API response")

    for prefer in ("en-Carter_man.pt", "en-Davis_man.pt"):
        if prefer in candidates:
            return prefer
    return candidates[0]


def _choose_device(requested: str) -> str:
    requested = requested.strip().lower()
    if requested:
        if requested == "mpx":
            return "mps"
        return requested

    import torch

    if torch.cuda.is_available():
        return "cuda"
    if torch.backends.mps.is_available():
        return "mps"
    return "cpu"


def _read_text(args: argparse.Namespace) -> str:
    if args.text_file:
        p = Path(args.text_file)
        return p.read_text(encoding="utf-8").strip()
    return args.text.strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--text", default="Hello from VibeVoice on PyTorch.")
    parser.add_argument("--text-file", default="")
    parser.add_argument("--out", default="out.wav")
    parser.add_argument("--device", default="", help="cuda | mps | cpu (default: auto)")
    parser.add_argument("--cfg-scale", type=float, default=1.5)
    parser.add_argument("--ddpm-steps", type=int, default=5)
    parser.add_argument(
        "--voice",
        default="",
        help="Voice preset name (e.g. en-Carter_man) or path to a .pt prompt file",
    )
    args = parser.parse_args()

    try:
        import copy
        import torch
        from transformers.utils import logging as hf_logging
        from vibevoice.modular.modeling_vibevoice_streaming_inference import (
            VibeVoiceStreamingForConditionalGenerationInference,
        )
        from vibevoice.processor.vibevoice_streaming_processor import VibeVoiceStreamingProcessor
    except Exception as e:
        print("Missing dependencies for VibeVoice PoC.", file=sys.stderr)
        print("Create a venv and run: pip install -r requirements.txt", file=sys.stderr)
        print(f"Import error: {e}", file=sys.stderr)
        return 1

    hf_logging.set_verbosity_info()

    device = _choose_device(args.device)
    print(f"Using device: {device}")

    text = _read_text(args)
    if not text:
        print("Error: empty text", file=sys.stderr)
        return 1

    cache_dir = _default_cache_dir()
    cache_dir.mkdir(parents=True, exist_ok=True)

    voice = args.voice.strip()
    if not voice:
        voice = _pick_default_voice_name()
        print(f"Using default voice preset: {voice}")

    voice_path = _resolve_voice_preset(voice, cache_dir)

    # dtype & attention implementation
    if device == "mps":
        load_dtype = torch.float32
        attn_impl = "sdpa"
    elif device == "cuda":
        load_dtype = torch.bfloat16
        attn_impl = "flash_attention_2"
    else:
        load_dtype = torch.float32
        attn_impl = "sdpa"

    print(f"Loading processor & model: {args.model}")
    print(f"torch_dtype={load_dtype}, attn_implementation={attn_impl}")

    processor = VibeVoiceStreamingProcessor.from_pretrained(args.model)

    # device_map logic mirrors upstream demo scripts
    if device == "mps":
        model = VibeVoiceStreamingForConditionalGenerationInference.from_pretrained(
            args.model,
            torch_dtype=load_dtype,
            attn_implementation=attn_impl,
            device_map=None,
        )
        model.to("mps")
        target_device = "mps"
    elif device == "cuda":
        model = VibeVoiceStreamingForConditionalGenerationInference.from_pretrained(
            args.model,
            torch_dtype=load_dtype,
            attn_implementation=attn_impl,
            device_map="cuda",
        )
        target_device = "cuda"
    else:
        model = VibeVoiceStreamingForConditionalGenerationInference.from_pretrained(
            args.model,
            torch_dtype=load_dtype,
            attn_implementation=attn_impl,
            device_map="cpu",
        )
        target_device = "cpu"

    model.eval()
    model.set_ddpm_inference_steps(num_steps=args.ddpm_steps)

    print(f"Loading voice prompt: {voice_path}")
    cached_prompt = torch.load(voice_path, map_location=target_device, weights_only=False)

    inputs = processor.process_input_with_cached_prompt(
        text=text,
        cached_prompt=cached_prompt,
        padding=True,
        return_tensors="pt",
        return_attention_mask=True,
    )
    for k, v in list(inputs.items()):
        if torch.is_tensor(v):
            inputs[k] = v.to(target_device)

    print(f"Generating... cfg_scale={args.cfg_scale}, ddpm_steps={args.ddpm_steps}")
    start = time.time()
    outputs = model.generate(
        **inputs,
        max_new_tokens=None,
        cfg_scale=args.cfg_scale,
        tokenizer=processor.tokenizer,
        generation_config={"do_sample": False},
        verbose=True,
        all_prefilled_outputs=copy.deepcopy(cached_prompt) if cached_prompt is not None else None,
    )
    elapsed = time.time() - start
    print(f"Generation finished in {elapsed:.2f}s")

    if not getattr(outputs, "speech_outputs", None) or outputs.speech_outputs[0] is None:
        print("No audio output generated.", file=sys.stderr)
        return 1

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    processor.save_audio(outputs.speech_outputs[0], output_path=str(out_path))
    print(f"WAV written: {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

