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
    ap.add_argument("--seed", type=int, default=0)
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
    if input_len <= 0 or num_samples <= 0:
        raise SystemExit("input-len and num-samples must be positive")

    rng = np.random.default_rng(args.seed)
    # Small weights to keep output in a reasonable range.
    W = (rng.standard_normal((input_len, num_samples), dtype=np.float32) * 0.02).astype(np.float32)
    B = (rng.standard_normal((1, num_samples), dtype=np.float32) * 0.01).astype(np.float32)

    inp = helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, input_len])
    out = helper.make_tensor_value_info("output", TensorProto.FLOAT, [1, num_samples])

    W_init = helper.make_tensor("W", TensorProto.FLOAT, [input_len, num_samples], W.flatten().tolist())
    B_init = helper.make_tensor("B", TensorProto.FLOAT, [1, num_samples], B.flatten().tolist())

    mm = helper.make_node("MatMul", ["input", "W"], ["mm"])
    add = helper.make_node("Add", ["mm", "B"], ["add"])
    tanh = helper.make_node("Tanh", ["add"], ["output"])

    graph = helper.make_graph([mm, add, tanh], "toy_tts", [inp], [out], initializer=[W_init, B_init])
    model = helper.make_model(graph, producer_name="llm-router-poc", opset_imports=[helper.make_opsetid("", 13)])
    model.ir_version = 10
    checker.check_model(model)

    with open(out_path, "wb") as f:
        f.write(model.SerializeToString())

    print(out_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

