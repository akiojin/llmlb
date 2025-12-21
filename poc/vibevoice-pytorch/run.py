#!/usr/bin/env python3
"""
VibeVoice TTS Runner for LLM Router Node

This script provides a CLI interface for VibeVoice text-to-speech synthesis.
It handles voice prompt downloading, caching, and actual TTS inference.

Environment variables:
  - HF_HOME: Hugging Face cache directory (default: ~/.cache/huggingface)
  - HF_TOKEN: Hugging Face token for authenticated downloads
"""
import argparse
import hashlib
import os
import sys
from pathlib import Path
from typing import Optional

import soundfile as sf
import torch

# Voice prompt URLs and configurations
VOICE_PROMPTS = {
    "Carter": {
        "url": "https://huggingface.co/microsoft/VibeVoice-Realtime-0.5B/resolve/main/audio_prompts/Carter.wav",
        "description": "Male voice, neutral tone",
    },
    "Nicole": {
        "url": "https://huggingface.co/microsoft/VibeVoice-Realtime-0.5B/resolve/main/audio_prompts/Nicole.wav",
        "description": "Female voice, neutral tone",
    },
    "Aria": {
        "url": "https://huggingface.co/microsoft/VibeVoice-Realtime-0.5B/resolve/main/audio_prompts/Aria.wav",
        "description": "Female voice, warm tone",
    },
    "Daphne": {
        "url": "https://huggingface.co/microsoft/VibeVoice-Realtime-0.5B/resolve/main/audio_prompts/Daphne.wav",
        "description": "Female voice, professional tone",
    },
    "Jessica": {
        "url": "https://huggingface.co/microsoft/VibeVoice-Realtime-0.5B/resolve/main/audio_prompts/Jessica.wav",
        "description": "Female voice, friendly tone",
    },
    "Ruby": {
        "url": "https://huggingface.co/microsoft/VibeVoice-Realtime-0.5B/resolve/main/audio_prompts/Ruby.wav",
        "description": "Female voice, energetic tone",
    },
}

DEFAULT_VOICE = "Carter"
DEFAULT_MODEL = "microsoft/VibeVoice-Realtime-0.5B"
DEFAULT_DEVICE = "mps"  # Apple Silicon
DEFAULT_DDPM_STEPS = 5
DEFAULT_CFG_SCALE = 1.5
DEFAULT_SAMPLE_RATE = 22050


def get_cache_dir() -> Path:
    """Get the cache directory for voice prompts."""
    hf_home = os.environ.get("HF_HOME", os.path.expanduser("~/.cache/huggingface"))
    cache_dir = Path(hf_home) / "vibevoice_prompts"
    cache_dir.mkdir(parents=True, exist_ok=True)
    return cache_dir


def download_voice_prompt(voice: str, force: bool = False) -> Path:
    """Download and cache a voice prompt file."""
    if voice not in VOICE_PROMPTS:
        available = ", ".join(VOICE_PROMPTS.keys())
        raise ValueError(f"Unknown voice: {voice}. Available: {available}")

    cache_dir = get_cache_dir()
    prompt_info = VOICE_PROMPTS[voice]
    url = prompt_info["url"]

    # Create a deterministic filename from the URL
    url_hash = hashlib.md5(url.encode()).hexdigest()[:8]
    cache_path = cache_dir / f"{voice}_{url_hash}.wav"

    if cache_path.exists() and not force:
        return cache_path

    print(f"Downloading voice prompt: {voice}...", file=sys.stderr)

    try:
        import urllib.request

        headers = {}
        hf_token = os.environ.get("HF_TOKEN")
        if hf_token:
            headers["Authorization"] = f"Bearer {hf_token}"

        req = urllib.request.Request(url, headers=headers)
        with urllib.request.urlopen(req, timeout=60) as response:
            with open(cache_path, "wb") as f:
                f.write(response.read())

        print(f"Voice prompt cached: {cache_path}", file=sys.stderr)
        return cache_path

    except Exception as e:
        raise RuntimeError(f"Failed to download voice prompt: {e}") from e


def load_model(model_id: str, device: str, dtype: torch.dtype):
    """Load the VibeVoice model."""
    try:
        from vibevoice import VibeVoice

        print(f"Loading VibeVoice model: {model_id}", file=sys.stderr)
        print(f"Device: {device}, dtype: {dtype}", file=sys.stderr)

        model = VibeVoice.from_pretrained(model_id)
        model = model.to(device=device, dtype=dtype)
        # Set to inference mode (same as model.eval())
        model.train(False)

        return model

    except ImportError:
        # Fallback: try using transformers directly
        print(
            "vibevoice package not found, attempting transformers fallback",
            file=sys.stderr,
        )
        return load_model_transformers(model_id, device, dtype)


def load_model_transformers(model_id: str, device: str, dtype: torch.dtype):
    """Fallback model loading using transformers."""
    from transformers import AutoModelForCausalLM, AutoProcessor

    print(f"Loading model via transformers: {model_id}", file=sys.stderr)

    model = AutoModelForCausalLM.from_pretrained(
        model_id,
        torch_dtype=dtype,
        trust_remote_code=True,
    )
    model = model.to(device)
    # Set to inference mode (same as model.eval())
    model.train(False)

    processor = AutoProcessor.from_pretrained(model_id, trust_remote_code=True)

    return {"model": model, "processor": processor}


def synthesize_vibevoice(
    model,
    text: str,
    voice_prompt_path: Path,
    ddpm_steps: int = DEFAULT_DDPM_STEPS,
    cfg_scale: float = DEFAULT_CFG_SCALE,
) -> tuple:
    """Synthesize speech using the VibeVoice model."""
    try:
        from vibevoice import VibeVoice

        if isinstance(model, VibeVoice):
            # Official vibevoice API
            audio, sr = model.tts(
                text=text,
                voice_prompt=str(voice_prompt_path),
                ddpm_steps=ddpm_steps,
                cfg_scale=cfg_scale,
            )
            return audio, sr

    except (ImportError, AttributeError):
        pass

    # Transformers fallback
    if isinstance(model, dict) and "model" in model and "processor" in model:
        return synthesize_transformers(
            model["model"],
            model["processor"],
            text,
            voice_prompt_path,
            ddpm_steps,
            cfg_scale,
        )

    raise RuntimeError("Unable to synthesize: incompatible model type")


def synthesize_transformers(
    model,
    processor,
    text: str,
    voice_prompt_path: Path,
    ddpm_steps: int,
    cfg_scale: float,
) -> tuple:
    """Synthesize speech using transformers fallback."""
    # Load voice prompt audio
    prompt_audio, prompt_sr = sf.read(voice_prompt_path)
    if len(prompt_audio.shape) > 1:
        prompt_audio = prompt_audio.mean(axis=1)  # Convert to mono

    # Prepare inputs
    inputs = processor(
        text=text,
        audio=prompt_audio,
        sampling_rate=prompt_sr,
        return_tensors="pt",
    )
    inputs = {k: v.to(model.device) for k, v in inputs.items()}

    # Generate
    with torch.no_grad():
        outputs = model.generate(
            **inputs,
            ddpm_steps=ddpm_steps,
            cfg_scale=cfg_scale,
        )

    # Extract audio
    if hasattr(outputs, "audio"):
        audio = outputs.audio.cpu().numpy()
    elif isinstance(outputs, torch.Tensor):
        audio = outputs.cpu().numpy()
    else:
        raise RuntimeError(f"Unexpected output type: {type(outputs)}")

    if audio.ndim > 1:
        audio = audio.squeeze()

    return audio, DEFAULT_SAMPLE_RATE


def get_device_dtype(device: str) -> tuple:
    """Get the appropriate device and dtype."""
    if device == "mps":
        # Apple Silicon
        if not torch.backends.mps.is_available():
            print("MPS not available, falling back to CPU", file=sys.stderr)
            device = "cpu"
        dtype = torch.float16
    elif device == "cuda":
        if not torch.cuda.is_available():
            print("CUDA not available, falling back to CPU", file=sys.stderr)
            device = "cpu"
        dtype = torch.float16
    else:
        device = "cpu"
        dtype = torch.float32

    return device, dtype


def list_voices():
    """Print available voices."""
    print("Available voices:")
    for name, info in VOICE_PROMPTS.items():
        print(f"  {name}: {info['description']}")


def main():
    parser = argparse.ArgumentParser(
        description="VibeVoice TTS Runner for LLM Router",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Basic usage
  %(prog)s --text "Hello world" --out output.wav

  # With specific voice
  %(prog)s --voice Nicole --text "Hello world" --out output.wav

  # List available voices
  %(prog)s --list-voices

Environment variables:
  HF_HOME    Hugging Face cache directory
  HF_TOKEN   Hugging Face token for authenticated downloads
        """,
    )

    parser.add_argument(
        "--model",
        default=DEFAULT_MODEL,
        help=f"Model ID (default: {DEFAULT_MODEL})",
    )
    parser.add_argument(
        "--device",
        default=DEFAULT_DEVICE,
        choices=["cpu", "cuda", "mps"],
        help=f"Device to use (default: {DEFAULT_DEVICE})",
    )
    parser.add_argument(
        "--ddpm-steps",
        type=int,
        default=DEFAULT_DDPM_STEPS,
        help=f"DDPM steps for diffusion (default: {DEFAULT_DDPM_STEPS})",
    )
    parser.add_argument(
        "--cfg-scale",
        type=float,
        default=DEFAULT_CFG_SCALE,
        help=f"CFG scale for generation (default: {DEFAULT_CFG_SCALE})",
    )
    parser.add_argument(
        "--voice",
        default=DEFAULT_VOICE,
        help=f"Voice to use (default: {DEFAULT_VOICE})",
    )
    parser.add_argument(
        "--voice-prompt",
        type=str,
        default=None,
        help="Path to custom voice prompt WAV file",
    )
    parser.add_argument(
        "--text",
        required=False,
        help="Text to synthesize",
    )
    parser.add_argument(
        "--out",
        default="output.wav",
        help="Output WAV file path (default: output.wav)",
    )
    parser.add_argument(
        "--list-voices",
        action="store_true",
        help="List available voices and exit",
    )
    parser.add_argument(
        "--force-download",
        action="store_true",
        help="Force re-download of voice prompts",
    )

    args = parser.parse_args()

    # Handle --list-voices
    if args.list_voices:
        list_voices()
        return 0

    # Validate required arguments
    if not args.text:
        parser.error("--text is required")

    try:
        # Get device and dtype
        device, dtype = get_device_dtype(args.device)
        print(f"Using device: {device}, dtype: {dtype}", file=sys.stderr)

        # Get voice prompt path
        if args.voice_prompt:
            voice_prompt_path = Path(args.voice_prompt)
            if not voice_prompt_path.exists():
                raise FileNotFoundError(f"Voice prompt not found: {voice_prompt_path}")
        else:
            voice_prompt_path = download_voice_prompt(
                args.voice, force=args.force_download
            )

        print(f"Voice prompt: {voice_prompt_path}", file=sys.stderr)

        # Load model
        model = load_model(args.model, device, dtype)

        # Synthesize
        print(f"Synthesizing: {args.text[:50]}...", file=sys.stderr)
        audio, sr = synthesize_vibevoice(
            model,
            args.text,
            voice_prompt_path,
            ddpm_steps=args.ddpm_steps,
            cfg_scale=args.cfg_scale,
        )

        # Write output
        output_path = Path(args.out)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        sf.write(str(output_path), audio, sr)

        print(f"Output written: {output_path} ({sr} Hz)", file=sys.stderr)
        return 0

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        import traceback

        traceback.print_exc(file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
