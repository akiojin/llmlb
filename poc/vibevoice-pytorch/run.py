#!/usr/bin/env python3
"""
Minimal VibeVoice-Realtime-0.5B inference (CPU, blocking) for PoC.
"""
import argparse
import os
from pathlib import Path

import soundfile as sf
import torch
from transformers import AutoModelForCausalLM, AutoTokenizer


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", default="microsoft/VibeVoice-Realtime-0.5B")
    parser.add_argument("--text", default="Hello from VibeVoice on PyTorch.")
    parser.add_argument("--out", default="out.wav")
    args = parser.parse_args()

    device = "cpu"
    dtype = torch.bfloat16

    print(f"Loading model {args.model} on {device} ({dtype}) ...")
    tokenizer = AutoTokenizer.from_pretrained(args.model)
    model = AutoModelForCausalLM.from_pretrained(
        args.model,
        torch_dtype=dtype,
        device_map=device,
    )

    inputs = tokenizer(
        args.text,
        return_tensors="pt",
    ).to(device)

    # NOTE: VibeVoice は text->acoustic tokens->diffusion で音声生成する専用ヘッドを持つ。
    # ここでは公式実装に倣った呼び出しが必要だが、Transformers の汎用 generate では動かない。
    # PoC 目的で forward を呼び、出力テンソル形状までを確認する。
    with torch.no_grad():
        out = model(**inputs)

    print("Forward pass OK. Model output keys:", list(out.keys()))
    # 実際の音声波形を得るには、公式リポの inference スクリプトにある
    # acoustic tokenizer + diffusion ヘッドの呼び出し手順を実装する必要がある。
    # ここではダミーの無音WAVを出力して「形だけ」整える。
    sr = 24000
    dummy = torch.zeros(sr * 1, dtype=torch.float32).cpu().numpy()
    sf.write(args.out, dummy, sr)
    print(f"Dummy WAV written: {args.out} (silence, {sr} Hz)")


if __name__ == "__main__":
    main()
