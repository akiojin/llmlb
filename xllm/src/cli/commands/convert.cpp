#include "cli/commands.h"

#include <filesystem>
#include <iostream>

#include "models/model_converter.h"
#include "utils/config.h"

namespace fs = std::filesystem;

namespace xllm::cli::commands {

int convert(const ConvertOptions& options) {
    if (options.format != "gguf") {
        std::cerr << "Error: unsupported format: " << options.format << "\n";
        return 1;
    }

    fs::path source = options.source;
    if (!fs::exists(source)) {
        std::cerr << "Error: source not found: " << source.string() << "\n";
        return 1;
    }

    auto config = loadNodeConfig();
    ModelConverter converter(config.models_dir.empty() ? fs::path(".") : fs::path(config.models_dir));

    std::string output;
    if (source.extension() == ".safetensors") {
        output = converter.convertSafetensorsToGguf(source, options.name);
    } else {
        output = converter.convertPyTorchToGguf(source, options.name);
    }

    if (output.empty()) {
        std::cerr << "Error: conversion failed\n";
        return 1;
    }

    std::cout << "Converted model saved to: " << output << "\n";
    return 0;
}

}  // namespace xllm::cli::commands
