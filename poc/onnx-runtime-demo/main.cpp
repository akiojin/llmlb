#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

#include <onnxruntime_cxx_api.h>

int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::cerr << "Usage: onnx_poc <model.onnx>\n";
        return 1;
    }

    const std::string model_path = argv[1];

    try {
        Ort::Env env{ORT_LOGGING_LEVEL_INFO, "onnx-poc"};
        Ort::SessionOptions opts;
        opts.SetIntraOpNumThreads(2);

#if defined(__APPLE__)
        // CoreML EP を最優先で登録。失敗しても CPU にフォールバックする。
        try {
            opts.AppendExecutionProvider("CoreMLExecutionProvider");
            std::cout << "CoreML EP enabled\n";
        } catch (const Ort::Exception& e) {
            std::cerr << "CoreML EP not available: " << e.what()
                      << " (fallback to XNNPACK/CPU)\n";
        }
#endif
        // XNNPACK を明示的に追加（CPU高速化）。失敗してもデフォルトCPUに落ちる。
        try {
            opts.AppendExecutionProvider("XnnpackExecutionProvider");
            std::cout << "XNNPACK EP enabled\n";
        } catch (const Ort::Exception& e) {
            std::cerr << "XNNPACK EP not available: " << e.what()
                      << " (fallback to default CPU)\n";
        }

        Ort::Session session{env, model_path.c_str(), opts};

        std::cout << "Loaded model: " << model_path << "\n";

        // 使用可能な EP を出力（優先順ではないが参考に表示）
        {
            auto providers = Ort::GetAvailableProviders();
            std::cout << "Available providers:\n";
            for (const auto& p : providers) {
                std::cout << "  - " << p << "\n";
            }
        }

        // 入力情報を出力
        size_t input_count = session.GetInputCount();
        std::cout << "Inputs: " << input_count << "\n";
        Ort::AllocatorWithDefaultOptions allocator;
        for (size_t i = 0; i < input_count; ++i) {
            auto name = session.GetInputNameAllocated(i, allocator);
            Ort::TypeInfo type_info = session.GetInputTypeInfo(i);
            auto tensor_info = type_info.GetTensorTypeAndShapeInfo();
            auto shape = tensor_info.GetShape();

            std::cout << "  [" << i << "] " << name.get() << " shape=(";
            for (size_t j = 0; j < shape.size(); ++j) {
                std::cout << shape[j];
                if (j + 1 < shape.size()) std::cout << ", ";
            }
            std::cout << ")\n";
        }

        std::cout << "Session initialization OK.\n";
    } catch (const Ort::Exception& e) {
        std::cerr << "ONNX Runtime error: " << e.what() << "\n";
        return 1;
    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << "\n";
        return 1;
    }

    return 0;
}
