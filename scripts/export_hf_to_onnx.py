#!/usr/bin/env python3

import argparse
import os
import subprocess
import sys
from pathlib import Path
from typing import Optional, Tuple


def eprint(msg: str) -> None:
    print(msg, file=sys.stderr, flush=True)


def parse_repo_spec(spec: str) -> Tuple[str, Optional[str]]:
    if "@" not in spec:
        return spec, None
    # repo@revision (best-effort; repo itself should not contain '@')
    repo, rev = spec.split("@", 1)
    if not repo:
        return spec, None
    return repo, rev or None


def try_download_aux_files(repo: str, revision: Optional[str], out_dir: str) -> None:
    try:
        from huggingface_hub import hf_hub_download
    except Exception:
        eprint("aux: huggingface_hub not available, skipping auxiliary downloads")
        return

    token = os.environ.get("HF_TOKEN") or None

    def dl(filename: str) -> None:
        try:
            src = hf_hub_download(
                repo_id=repo,
                filename=filename,
                revision=revision or "main",
                token=token,
            )
        except Exception:
            return
        try:
            dst = os.path.join(out_dir, filename)
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            # Copy (not symlink) so router cache is self-contained.
            with open(src, "rb") as rf, open(dst, "wb") as wf:
                wf.write(rf.read())
        except Exception:
            return

    for f in [
        "config.json",
        "generation_config.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "special_tokens_map.json",
        "chat_template.jinja",
    ]:
        dl(f)


def snapshot_if_needed(repo: str, revision: Optional[str]) -> str:
    if not revision:
        return repo

    try:
        from huggingface_hub import snapshot_download
    except Exception as e:
        raise RuntimeError(f"snapshot_download unavailable (huggingface_hub missing?): {e}") from e

    token = os.environ.get("HF_TOKEN") or None
    local_dir = snapshot_download(repo_id=repo, revision=revision, token=token)
    return local_dir


def ensure_tokenizer_json(model_id_or_path: str, out_dir: str) -> None:
    """
    Ensure Hugging Face `tokenizer.json` exists in out_dir.

    Node-side tokenizer uses the Rust `tokenizers` crate which expects tokenizer.json.
    Some repos do not ship tokenizer.json (or we might have skipped aux download),
    so we generate it via Transformers as a best-effort fallback.
    """
    tok_path = os.path.join(out_dir, "tokenizer.json")
    if os.path.exists(tok_path):
        return

    try:
        from transformers import AutoTokenizer
    except Exception as e:
        eprint(f"aux: transformers not available, cannot generate tokenizer.json ({e})")
        return

    try:
        tok = AutoTokenizer.from_pretrained(model_id_or_path, use_fast=True)
    except Exception as e:
        eprint(f"aux: failed to load fast tokenizer, cannot generate tokenizer.json ({e})")
        return

    # Try standard save_pretrained first.
    try:
        tok.save_pretrained(out_dir)
    except Exception:
        pass

    if os.path.exists(tok_path):
        return

    # Fallback: backend_tokenizer.save (fast tokenizers)
    try:
        backend = getattr(tok, "backend_tokenizer", None)
        if backend is not None:
            backend.save(tok_path)
    except Exception:
        return


def export_with_optimum_cli(model_id_or_path: str, out_dir: str) -> None:
    # Use Optimum exporter (recommended by Transformers). It is more robust than
    # the deprecated transformers.onnx exporter for modern Torch/Transformers.
    cmd = [
        sys.executable,
        "-m",
        "optimum.exporters.onnx",
        "--model",
        model_id_or_path,
        "--task",
        "text-generation-with-past",
        "--library",
        "transformers",
        out_dir,
    ]

    trust_remote_code = os.environ.get("LLM_CONVERT_TRUST_REMOTE_CODE", "").strip().lower()
    if trust_remote_code in ("1", "true", "yes", "on"):
        # WARNING: This executes arbitrary code from the model repository.
        cmd.insert(-1, "--trust-remote-code")

    env = os.environ.copy()
    env["PYTHONUNBUFFERED"] = "1"
    subprocess.check_call(cmd, env=env)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--remote", action="store_true", help="(compat) ignored; always uses HF repo id")
    ap.add_argument("--outfile", required=True, help="Path to write model.onnx (directory used for output)")
    ap.add_argument("model", help="Hugging Face repo id (optionally repo@revision)")
    args = ap.parse_args()

    repo, revision = parse_repo_spec(args.model)
    out_path = os.path.abspath(args.outfile)
    out_dir = os.path.dirname(out_path)
    os.makedirs(out_dir, exist_ok=True)

    eprint("10%")
    try_download_aux_files(repo, revision, out_dir)

    try:
        model_id_or_path = snapshot_if_needed(repo, revision)
        eprint("50%")
        export_with_optimum_cli(model_id_or_path, out_dir)
        ensure_tokenizer_json(model_id_or_path, out_dir)
        eprint("90%")
    except Exception as e:
        eprint(f"export failed: {e}")
        return 1

    # Ensure the main onnx file exists at args.outfile (best-effort).
    if not os.path.exists(out_path):
        # Transformers exporter typically writes model.onnx, but older versions may use a different name.
        candidates = [f for f in os.listdir(out_dir) if f.endswith(".onnx")]
        if candidates:
            src = os.path.join(out_dir, candidates[0])
            os.replace(src, out_path)

    if not os.path.exists(out_path):
        eprint("export failed: model.onnx not found after export")
        return 1

    eprint("100%")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
