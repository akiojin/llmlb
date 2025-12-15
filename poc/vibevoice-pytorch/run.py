#!/usr/bin/env python3
"""
VibeVoice-Realtime-0.5B minimal TTS PoC (PyTorch, streaming inference).

Notes:
- VibeVoice-Realtime-0.5B は公式実装（microsoft/VibeVoice）に依存します。
- この PoC は ONNX 変換ではなく PyTorch 推論です。
- 音声プロンプトは `.pt`（埋め込み形式の voice prompt）を使用します。
"""

from __future__ import annotations

import argparse
import copy
import os
import sys
import time
import urllib.request
from pathlib import Path

DEFAULT_MODEL = "microsoft/VibeVoice-Realtime-0.5B"
DEFAULT_VOICE = "Carter"

VOICE_PROMPT_FILES = [
    "de-Spk0_man",
    "de-Spk1_woman",
    "en-Carter_man",
    "en-Davis_man",
    "en-Emma_woman",
    "en-Frank_man",
    "en-Grace_woman",
    "en-Mike_man",
    "fr-Spk0_man",
    "fr-Spk1_woman",
    "in-Samuel_man",
    "it-Spk0_woman",
    "it-Spk1_man",
    "jp-Spk0_man",
    "jp-Spk1_woman",
    "kr-Spk0_woman",
    "kr-Spk1_man",
    "nl-Spk0_man",
    "nl-Spk1_woman",
    "pl-Spk0_man",
    "pl-Spk1_woman",
    "pt-Spk0_woman",
    "pt-Spk1_man",
    "sp-Spk0_woman",
    "sp-Spk1_man",
]

VOICE_ALIASES = {
    "carter": "en-Carter_man.pt",
    "davis": "en-Davis_man.pt",
    "emma": "en-Emma_woman.pt",
    "frank": "en-Frank_man.pt",
    "grace": "en-Grace_woman.pt",
    "mike": "en-Mike_man.pt",
    "samuel": "in-Samuel_man.pt",
    # Language-tagged shortcuts
    "jp-man": "jp-Spk0_man.pt",
    "jp-woman": "jp-Spk1_woman.pt",
    "ja-man": "jp-Spk0_man.pt",
    "ja-woman": "jp-Spk1_woman.pt",
}

VOICE_RAW_BASE_URL = "https://raw.githubusercontent.com/microsoft/VibeVoice/main/demo/voices/streaming_model"


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


def _read_text(args: argparse.Namespace) -> str:
    if args.text_file:
        p = Path(args.text_file)
        return p.read_text(encoding="utf-8")
    return args.text


def _normalize_text(text: str) -> str:
    # Keep it close to the official demo: normalize curly quotes, etc.
    return (
        text.strip()
        .replace("’", "'")
        .replace("“", '"')
        .replace("”", '"')
    )


def _build_voice_map() -> dict[str, str]:
    # base name -> filename
    mapping: dict[str, str] = {base.lower(): f"{base}.pt" for base in VOICE_PROMPT_FILES}
    mapping.update(VOICE_ALIASES)
    return mapping


def _resolve_voice_prompt(voice: str, cache_dir: Path) -> Path:
    voice = (voice or "").strip()
    if voice.lower() in ("", "default"):
        voice = DEFAULT_VOICE

    p = Path(voice)
    if p.exists() and p.is_file():
        if p.suffix.lower() != ".pt":
            raise RuntimeError(f"Voice prompt must be a .pt file: {p}")
        return p

    voice_map = _build_voice_map()

    key = voice.lower()
    if key.endswith(".pt"):
        key = key[:-3]
    filename = voice_map.get(key)
    if not filename:
        # Allow full preset base name (case-sensitive-ish) without mapping.
        # Example: "en-Emma_woman" / "jp-Spk1_woman"
        if voice in VOICE_PROMPT_FILES:
            filename = f"{voice}.pt"
        else:
            available = sorted({DEFAULT_VOICE.lower(), *voice_map.keys()})
            raise RuntimeError(
                "Unknown voice preset. "
                f"voice={voice!r}. "
                "Try one of: " + ", ".join(available[:20]) + (" ..." if len(available) > 20 else "")
            )

    cache_dir.mkdir(parents=True, exist_ok=True)
    dst = cache_dir / filename
    if dst.exists() and dst.stat().st_size > 0:
        return dst

    url = f"{VOICE_RAW_BASE_URL}/{filename}"
    tmp = dst.with_suffix(dst.suffix + ".tmp")
    try:
        with urllib.request.urlopen(url) as r, open(tmp, "wb") as f:
            f.write(r.read())
        tmp.replace(dst)
    finally:
        try:
            if tmp.exists():
                tmp.unlink()
        except Exception:
            pass
    return dst


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--text", default="Hello from VibeVoice on PyTorch.")
    parser.add_argument("--text-file", default="")
    parser.add_argument("--out", default="out.wav")
    parser.add_argument("--device", default="auto", help="auto | cuda | mps (default: auto)")
    parser.add_argument("--require-gpu", action="store_true", help="Fail if cuda/mps is not available")
    parser.add_argument("--cfg-scale", type=float, default=1.5)
    parser.add_argument("--ddpm-steps", type=int, default=5)
    parser.add_argument("--voice", default="default", help="Voice preset name (default: Carter)")
    parser.add_argument("--voice-cache-dir", default="", help="Directory to cache voice prompts (.pt)")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    try:
        import torch
        from transformers.utils import logging as hf_logging
        from vibevoice.modular.modeling_vibevoice_streaming_inference import (
            VibeVoiceStreamingForConditionalGenerationInference,
        )
        from vibevoice.processor.vibevoice_streaming_processor import VibeVoiceStreamingProcessor
    except Exception as e:
        print("Missing dependencies for VibeVoice PoC.", file=sys.stderr)
        print(
            "Create a venv and run: pip install -r requirements.txt && pip install --no-deps git+https://github.com/microsoft/VibeVoice.git",
            file=sys.stderr,
        )
        print(f"Import error: {e}", file=sys.stderr)
        return 1

    hf_logging.set_verbosity_info() if args.verbose else hf_logging.set_verbosity_error()

    try:
        device = _choose_device(args.device, args.require_gpu)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

    print(f"Using device: {device}")

    if args.require_gpu and device == "mps" and os.environ.get("PYTORCH_ENABLE_MPS_FALLBACK") == "1":
        print(
            "Error: PYTORCH_ENABLE_MPS_FALLBACK=1 is set. CPU fallback is not allowed when --require-gpu is used.",
            file=sys.stderr,
        )
        return 1

    text = _normalize_text(_read_text(args))
    if not text:
        print("Error: empty text", file=sys.stderr)
        return 1

    cache_dir = Path(args.voice_cache_dir) if args.voice_cache_dir else (Path.home() / ".cache" / "llm-router" / "vibevoice_voices")
    try:
        voice_prompt_path = _resolve_voice_prompt(args.voice, cache_dir)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

    if device == "mps":
        load_dtype = torch.float32
        attn_impl_primary = "sdpa"
    elif device == "cuda":
        load_dtype = torch.bfloat16
        attn_impl_primary = "flash_attention_2"
    else:
        load_dtype = torch.float32
        attn_impl_primary = "sdpa"

    print(f"Loading processor & model: {args.model}")
    print(f"torch_dtype={load_dtype}, attn_implementation={attn_impl_primary}")

    processor = VibeVoiceStreamingProcessor.from_pretrained(args.model)

    try:
        if device == "mps":
            model = VibeVoiceStreamingForConditionalGenerationInference.from_pretrained(
                args.model,
                torch_dtype=load_dtype,
                attn_implementation=attn_impl_primary,
                device_map=None,
            )
            model.to("mps")
        elif device == "cuda":
            model = VibeVoiceStreamingForConditionalGenerationInference.from_pretrained(
                args.model,
                torch_dtype=load_dtype,
                device_map="cuda",
                attn_implementation=attn_impl_primary,
            )
        else:
            model = VibeVoiceStreamingForConditionalGenerationInference.from_pretrained(
                args.model,
                torch_dtype=load_dtype,
                device_map="cpu",
                attn_implementation=attn_impl_primary,
            )
    except Exception as e:
        if attn_impl_primary == "flash_attention_2":
            print(f"Warning: flash_attention_2 failed ({type(e).__name__}: {e}). Falling back to SDPA.", file=sys.stderr)
            model = VibeVoiceStreamingForConditionalGenerationInference.from_pretrained(
                args.model,
                torch_dtype=load_dtype,
                device_map=("cuda" if device == "cuda" else "cpu"),
                attn_implementation="sdpa",
            )
        else:
            raise

    model.eval()
    model.set_ddpm_inference_steps(num_steps=args.ddpm_steps)

    target_device = device if device != "cpu" else "cpu"
    try:
        try:
            all_prefilled_outputs = torch.load(voice_prompt_path, map_location=target_device, weights_only=False)
        except TypeError:
            all_prefilled_outputs = torch.load(voice_prompt_path, map_location=target_device)
    except Exception as e:
        print(f"Error: failed to load voice prompt: {voice_prompt_path} ({e})", file=sys.stderr)
        return 1

    inputs = processor.process_input_with_cached_prompt(
        text=text,
        cached_prompt=all_prefilled_outputs,
        padding=True,
        return_tensors="pt",
        return_attention_mask=True,
    )
    for k, v in inputs.items():
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
        verbose=args.verbose,
        all_prefilled_outputs=copy.deepcopy(all_prefilled_outputs) if all_prefilled_outputs is not None else None,
    )
    elapsed = time.time() - start
    print(f"Generation finished in {elapsed:.2f}s")

    speech_outputs = getattr(outputs, "speech_outputs", None)
    if not speech_outputs:
        print("No audio output generated.", file=sys.stderr)
        return 1

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)

    first = speech_outputs[0] if isinstance(speech_outputs, list) else speech_outputs
    if first is None:
        print("No audio output generated.", file=sys.stderr)
        return 1

    saved = processor.save_audio(first, output_path=str(out_path))
    print(f"WAV written: {saved[0] if isinstance(saved, list) and saved else out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
