#!/usr/bin/env python3

import argparse
import os


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--out",
        default="/tmp/llm_router_audio_poc_models/toy_tts.onnx",
        help="Output path for the generated ONNX model",
    )
    ap.add_argument("--input-len", type=int, default=32)
    ap.add_argument("--num-samples", type=int, default=8000)
    ap.add_argument("--sample-rate", type=int, default=16000)
    ap.add_argument("--tone-hz", type=float, default=440.0)
    ap.add_argument("--amp", type=float, default=0.2)
    ap.add_argument("--fade-ms", type=float, default=10.0)
    args = ap.parse_args()

    try:
        from onnx import TensorProto, checker, helper
        import numpy as np
    except Exception as e:
        raise SystemExit(
            f"Missing python deps for toy TTS model generation: {e}\n"
            "Install with: python3 -m pip install onnx numpy"
        )

    out_path = os.path.abspath(args.out)
    os.makedirs(os.path.dirname(out_path), exist_ok=True)

    input_len = int(args.input_len)
    num_samples = int(args.num_samples)
    sample_rate = int(args.sample_rate)
    tone_hz = float(args.tone_hz)
    amp = float(args.amp)
    fade_ms = float(args.fade_ms)
    if input_len <= 0 or num_samples <= 0:
        raise SystemExit("input-len and num-samples must be positive")
    if sample_rate <= 0:
        raise SystemExit("sample-rate must be positive")
    if amp <= 0:
        raise SystemExit("amp must be positive")
    if fade_ms < 0:
        raise SystemExit("fade-ms must be non-negative")

    # Generate a simple audible sine tone so users can confirm audio output.
    t = np.arange(num_samples, dtype=np.float32) / float(sample_rate)
    wave = (amp * np.sin(2.0 * np.pi * tone_hz * t)).astype(np.float32)
    fade_samples = int(round(sample_rate * (fade_ms / 1000.0)))
    if fade_samples > 0 and fade_samples * 2 < num_samples:
        ramp = np.linspace(0.0, 1.0, fade_samples, dtype=np.float32)
        wave[:fade_samples] *= ramp
        wave[-fade_samples:] *= ramp[::-1]

    # Weight matrix where the first feature maps directly to the tone.
    # The node PoC sets features[0] = 1.0, so output becomes the tone waveform.
    W = np.zeros((input_len, num_samples), dtype=np.float32)
    W[0, :] = wave
    B = np.zeros((1, num_samples), dtype=np.float32)

    inp = helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, input_len])
    out = helper.make_tensor_value_info("output", TensorProto.FLOAT, [1, num_samples])

    W_init = helper.make_tensor("W", TensorProto.FLOAT, [input_len, num_samples], W.flatten().tolist())
    B_init = helper.make_tensor("B", TensorProto.FLOAT, [1, num_samples], B.flatten().tolist())

    mm = helper.make_node("MatMul", ["input", "W"], ["mm"])
    add = helper.make_node("Add", ["mm", "B"], ["output"])

    graph = helper.make_graph([mm, add], "toy_tts", [inp], [out], initializer=[W_init, B_init])
    model = helper.make_model(graph, producer_name="llm-router-poc", opset_imports=[helper.make_opsetid("", 13)])
    model.ir_version = 10
    checker.check_model(model)

    with open(out_path, "wb") as f:
        f.write(model.SerializeToString())

    print(out_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
