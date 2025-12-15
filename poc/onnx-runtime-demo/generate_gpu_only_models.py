#!/usr/bin/env python3

import argparse
import os


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--out-dir",
        default="/tmp/onnx_poc_models",
        help="Output directory for generated ONNX models",
    )
    ap.add_argument(
        "--seed",
        type=int,
        default=0,
        help="Seed for random initializer generation",
    )
    args = ap.parse_args()

    out_dir = os.path.abspath(args.out_dir)
    os.makedirs(out_dir, exist_ok=True)

    try:
        from onnx import TensorProto, checker, helper
        import numpy as np
    except Exception as e:
        raise SystemExit(
            f"Missing python deps for PoC model generation: {e}\n"
            "Install with: python3 -m pip install onnx numpy"
        )

    def write(model, name: str) -> str:
        model.ir_version = 10
        checker.check_model(model)
        path = os.path.join(out_dir, name)
        with open(path, "wb") as f:
            f.write(model.SerializeToString())
        return path

    # A small numeric model expected to be fully handled by CoreML EP.
    inp = helper.make_tensor_value_info("input", TensorProto.FLOAT, [1, 3, 32, 32])
    out = helper.make_tensor_value_info("output", TensorProto.FLOAT, [1, 4, 30, 30])
    rng = np.random.default_rng(args.seed)
    W = rng.standard_normal((4, 3, 3, 3), dtype=np.float32)
    B = rng.standard_normal((4,), dtype=np.float32)
    W_init = helper.make_tensor("W", TensorProto.FLOAT, [4, 3, 3, 3], W.flatten().tolist())
    B_init = helper.make_tensor("B", TensorProto.FLOAT, [4], B.flatten().tolist())
    conv = helper.make_node("Conv", ["input", "W", "B"], ["output"], strides=[1, 1], pads=[0, 0, 0, 0])
    graph = helper.make_graph([conv], "g_conv", [inp], [out], initializer=[W_init, B_init])
    conv_model = helper.make_model(graph, producer_name="llm-router-poc", opset_imports=[helper.make_opsetid("", 13)])
    conv_path = write(conv_model, "conv.onnx")

    # A model that should NOT be handled by CoreML EP (string tensors).
    # With CPU EP fallback disabled, session creation must fail.
    sinp = helper.make_tensor_value_info("input", TensorProto.STRING, [1])
    sout = helper.make_tensor_value_info("output", TensorProto.STRING, [1])
    sident = helper.make_node("Identity", ["input"], ["output"])
    sgraph = helper.make_graph([sident], "g_string_identity", [sinp], [sout])
    s_model = helper.make_model(
        sgraph, producer_name="llm-router-poc", opset_imports=[helper.make_opsetid("", 13)]
    )
    s_path = write(s_model, "string_identity.onnx")

    print(conv_path)
    print(s_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
