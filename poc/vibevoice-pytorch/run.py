#!/usr/bin/env python3
"""
VibeVoice-Realtime-0.5B minimal TTS PoC (PyTorch).

Notes:
- VibeVoice は現状 ONNX 変換が難しいため、このPoCは PyTorch 推論です。
- 入力テキストは `Speaker 0: ...` のようなスクリプト形式を推奨します。
 （通常の文章を渡した場合は `Speaker 0:` を自動付与します）
"""

from __future__ import annotations

import argparse
import os
import re
import sys
import time
from pathlib import Path

DEFAULT_MODEL = "microsoft/VibeVoice-Realtime-0.5B"


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


def _normalize_script(text: str) -> str:
    script = text.strip()
    if not script:
        return ""

    if re.search(r"(?im)^Speaker\\s+\\d+\\s*:", script):
        return script

    # Plain text -> single speaker script.
    return f"Speaker 0: {script}"


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
    parser.add_argument("--voice", dest="voice_samples", action="append", default=[],
                        help="Voice sample audio path (repeatable; optional)")
    parser.add_argument("--voice-sample", dest="voice_samples", action="append", default=[],
                        help=argparse.SUPPRESS)
    parser.add_argument("--show-progress", action="store_true", help="Show generation progress bar")
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    try:
        import torch
        from transformers.utils import logging as hf_logging
        from vibevoice.modular.modeling_vibevoice_inference import (
            VibeVoiceForConditionalGenerationInference,
        )
        from vibevoice.processor.vibevoice_processor import VibeVoiceProcessor
    except Exception as e:
        print("Missing dependencies for VibeVoice PoC.", file=sys.stderr)
        print(
            "Create a venv and run: pip install -r requirements.txt && pip install --no-deps vibevoice==0.0.1",
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

    script = _normalize_script(_read_text(args))
    if not script:
        print("Error: empty text", file=sys.stderr)
        return 1

    voice_samples = [v for v in args.voice_samples if v.strip()]
    if voice_samples:
        for p in voice_samples:
            if not Path(p).exists():
                print(f"Error: voice sample not found: {p}", file=sys.stderr)
                return 1
    else:
        print(
            "Error: VibeVoice requires a voice sample. Pass --voice /path/to/sample.wav (or repeat for multiple).",
            file=sys.stderr,
        )
        return 1

    if device == "cuda":
        load_dtype = torch.bfloat16
    else:
        load_dtype = torch.float32
    attn_impl = "sdpa"

    print(f"Loading processor & model: {args.model}")
    print(f"torch_dtype={load_dtype}, attn_implementation={attn_impl}")

    processor = VibeVoiceProcessor.from_pretrained(args.model)
    model = VibeVoiceForConditionalGenerationInference.from_pretrained(
        args.model,
        torch_dtype=load_dtype,
        attn_implementation=attn_impl,
    )
    model.to(device)

    model.eval()
    model.set_ddpm_inference_steps(num_steps=args.ddpm_steps)

    inputs = processor(
        text=script,
        voice_samples=voice_samples if voice_samples else None,
        padding=True,
        return_tensors="pt",
        return_attention_mask=True,
    )
    for k, v in list(inputs.items()):
        if torch.is_tensor(v):
            inputs[k] = v.to(device)

    print(f"Generating... cfg_scale={args.cfg_scale}, ddpm_steps={args.ddpm_steps}")
    start = time.time()
    outputs = model.generate(
        cfg_scale=args.cfg_scale,
        tokenizer=processor.tokenizer,
        verbose=args.verbose,
        show_progress_bar=args.show_progress,
        input_ids=inputs.get("input_ids"),
        attention_mask=inputs.get("attention_mask"),
        speech_tensors=inputs.get("speech_tensors"),
        speech_masks=inputs.get("speech_masks"),
        speech_input_mask=inputs.get("speech_input_mask"),
        parsed_scripts=inputs.get("parsed_scripts"),
        all_speakers_list=inputs.get("all_speakers_list"),
    )
    elapsed = time.time() - start
    print(f"Generation finished in {elapsed:.2f}s")

    speech_outputs = getattr(outputs, "speech_outputs", None)
    if not speech_outputs:
        print("No audio output generated.", file=sys.stderr)
        return 1

    def _to_1d_np(a):
        import numpy as np

        if torch.is_tensor(a):
            a = a.float().detach().cpu().numpy()
        a = np.asarray(a)
        return a.squeeze()

    # The model may return multiple segments. Stitch into a single waveform.
    parts = []
    if isinstance(speech_outputs, list):
        for p in speech_outputs:
            if p is None:
                continue
            parts.append(_to_1d_np(p))
    else:
        parts = [_to_1d_np(speech_outputs)]

    if not parts:
        print("No audio output generated.", file=sys.stderr)
        return 1

    try:
        import numpy as np
        import soundfile as sf
    except Exception as e:
        print("Missing dependencies to save audio.", file=sys.stderr)
        print("Install: pip install soundfile numpy", file=sys.stderr)
        print(f"Import error: {e}", file=sys.stderr)
        return 1

    sr = getattr(getattr(processor, "audio_processor", None), "sampling_rate", 24000)
    gap = np.zeros(int(sr * 0.12), dtype=np.float32)
    stitched = parts[0]
    for p in parts[1:]:
        stitched = np.concatenate([stitched, gap, p], axis=0)

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    sf.write(str(out_path), stitched, sr)
    print(f"WAV written: {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
