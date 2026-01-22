// SPEC-58378000: node show command
// Shows model metadata and information

#include "utils/cli.h"
#include "cli/cli_client.h"
#include "cli/ollama_compat.h"
#include <iostream>
#include <iomanip>

namespace xllm {
namespace cli {
namespace commands {

/// Execute the 'node show' command
/// @param options Show options (model name, info type)
/// @return Exit code (0=success, 1=error, 2=connection error)
int show(const ShowOptions& options) {
    if (options.model.empty()) {
        std::cerr << "Error: model name required" << std::endl;
        return 1;
    }

    // Check if this is an ollama model reference
    if (OllamaCompat::hasOllamaPrefix(options.model)) {
        OllamaCompat ollama;
        std::string model_name = OllamaCompat::stripOllamaPrefix(options.model);
        auto info = ollama.getModel(model_name);

        if (!info) {
            std::cerr << "Error: ollama model not found: " << model_name << std::endl;
            return 1;
        }

        std::cout << "Name: " << info->name << std::endl;
        std::cout << "Source: ollama (read-only)" << std::endl;
        std::cout << "Digest: " << info->blob_digest << std::endl;
        std::cout << "Size: " << info->size_bytes << " bytes" << std::endl;
        std::cout << "Blob Path: " << info->blob_path << std::endl;
        std::cout << "Manifest: " << info->manifest_path << std::endl;
        return 0;
    }

    // Create CLI client for server models
    auto client = std::make_shared<CliClient>();

    // Check server connection
    if (!client->isServerRunning()) {
        std::cerr << "Error: Could not connect to xllm server" << std::endl;
        std::cerr << "Start the server with: xllm serve" << std::endl;
        return 2;
    }

    // Get model info from server
    auto result = client->showModel(options.model);
    if (!result.ok()) {
        std::cerr << "Error: " << result.error_message << std::endl;
        return 1;
    }

    // Handle specific info type flags
    if (result.data) {
        const auto& data = *result.data;

        if (options.license_only) {
            if (data.contains("license")) {
                std::cout << data["license"].get<std::string>() << std::endl;
            } else {
                std::cerr << "Error: license not available for this model" << std::endl;
                return 1;
            }
            return 0;
        }

        if (options.modelfile_only) {
            if (data.contains("modelfile")) {
                std::cout << data["modelfile"].get<std::string>() << std::endl;
            } else {
                std::cerr << "Error: modelfile not available for this model" << std::endl;
                return 1;
            }
            return 0;
        }

        if (options.parameters_only) {
            if (data.contains("parameters")) {
                std::cout << data["parameters"].dump(2) << std::endl;
            } else {
                std::cerr << "Error: parameters not available for this model" << std::endl;
                return 1;
            }
            return 0;
        }

        if (options.template_only) {
            if (data.contains("template")) {
                std::cout << data["template"].get<std::string>() << std::endl;
            } else {
                std::cerr << "Error: template not available for this model" << std::endl;
                return 1;
            }
            return 0;
        }

        if (options.system_only) {
            if (data.contains("system")) {
                std::cout << data["system"].get<std::string>() << std::endl;
            } else {
                std::cerr << "Error: system prompt not available for this model" << std::endl;
                return 1;
            }
            return 0;
        }
    }

    // Default: show all metadata
    if (result.data) {
        const auto& data = *result.data;

        if (data.contains("modelfile")) {
            std::cout << "Modelfile:" << std::endl;
            std::cout << data["modelfile"].get<std::string>() << std::endl;
            std::cout << std::endl;
        }

        if (data.contains("parameters")) {
            std::cout << "Parameters:" << std::endl;
            std::cout << data["parameters"].dump(2) << std::endl;
            std::cout << std::endl;
        }

        if (data.contains("template")) {
            std::cout << "Template:" << std::endl;
            std::cout << data["template"].get<std::string>() << std::endl;
            std::cout << std::endl;
        }

        if (data.contains("details")) {
            std::cout << "Details:" << std::endl;
            const auto& details = data["details"];
            if (details.contains("parent_model")) {
                std::cout << "  Parent Model: " << details["parent_model"].get<std::string>() << std::endl;
            }
            if (details.contains("format")) {
                std::cout << "  Format: " << details["format"].get<std::string>() << std::endl;
            }
            if (details.contains("family")) {
                std::cout << "  Family: " << details["family"].get<std::string>() << std::endl;
            }
            if (details.contains("parameter_size")) {
                std::cout << "  Parameter Size: " << details["parameter_size"].get<std::string>() << std::endl;
            }
            if (details.contains("quantization_level")) {
                std::cout << "  Quantization: " << details["quantization_level"].get<std::string>() << std::endl;
            }
        }

        // HuggingFace metadata
        if (data.contains("huggingface")) {
            std::cout << std::endl << "HuggingFace Metadata:" << std::endl;
            const auto& hf = data["huggingface"];
            if (hf.contains("repo_id")) {
                std::cout << "  Repository: " << hf["repo_id"].get<std::string>() << std::endl;
            }
            if (hf.contains("author")) {
                std::cout << "  Author: " << hf["author"].get<std::string>() << std::endl;
            }
            if (hf.contains("downloads")) {
                std::cout << "  Downloads: " << hf["downloads"].get<uint64_t>() << std::endl;
            }
            if (hf.contains("likes")) {
                std::cout << "  Likes: " << hf["likes"].get<uint64_t>() << std::endl;
            }
        }
    }

    return 0;
}

}  // namespace commands
}  // namespace cli
}  // namespace xllm
