#!/usr/bin/env python3

import argparse
import os
import subprocess
import sys


def eprint(msg: str) -> None:
    print(msg, file=sys.stderr, flush=True)


def parse_repo_spec(spec: str) -> tuple[str, str | None]:
    if "@" not in spec:
        return spec, None
    # repo@revision (best-effort; repo itself should not contain '@')
    repo, rev = spec.split("@", 1)
    if not repo:
        return spec, None
    return repo, rev or None


def try_download_aux_files(repo: str, revision: str | None, out_dir: str) -> None:
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


def export_with_transformers_onnx(repo: str, revision: str | None, out_dir: str) -> None:
    cmd = [
        sys.executable,
        "-m",
        "transformers.onnx",
        "--model",
        repo,
        "--feature",
        "causal-lm",
        "--use_external_data_format",
        out_dir,
    ]
    # transformers.onnx supports --revision in newer versions; best-effort.
    if revision:
        cmd.extend(["--revision", revision])

    env = os.environ.copy()
    env["PYTHONUNBUFFERED"] = "1"

    eprint("50%")
    subprocess.check_call(cmd, env=env)
    eprint("90%")


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
        export_with_transformers_onnx(repo, revision, out_dir)
    except subprocess.CalledProcessError as e:
        eprint(f"export failed: {e}")
        return e.returncode or 1
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

