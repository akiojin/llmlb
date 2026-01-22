// SPEC-58378000: node ps command
// Lists running models with extended metrics

#include "utils/cli.h"
#include "cli/cli_client.h"
#include <iostream>
#include <iomanip>
#include <sstream>

namespace xllm {
namespace cli {
namespace commands {

/// Execute the 'node ps' command
/// @return Exit code (0=success, 1=error, 2=connection error)
int ps() {
    // Create CLI client
    auto client = std::make_shared<CliClient>();

    // Check server connection
    if (!client->isServerRunning()) {
        std::cerr << "Error: Could not connect to allm server" << std::endl;
        std::cerr << "Start the server with: allm serve" << std::endl;
        return 2;
    }

    // Get running models
    auto result = client->listRunningModels();
    if (!result.ok()) {
        std::cerr << "Error: " << result.error_message << std::endl;
        return 1;
    }

    // Print header (ollama-compatible + extended metrics)
    std::cout << std::left
              << std::setw(40) << "NAME"
              << std::setw(15) << "ID"
              << std::setw(12) << "SIZE"
              << std::setw(15) << "PROCESSOR"
              << std::setw(10) << "VRAM"
              << std::setw(8) << "TEMP"
              << std::setw(12) << "REQS"
              << std::setw(15) << "UNTIL"
              << std::endl;

    // Print models
    if (result.data && result.data->contains("models")) {
        for (const auto& model : (*result.data)["models"]) {
            std::string name = model.value("name", "unknown");
            std::string id = model.value("digest", "").substr(0, 12);
            uint64_t size = model.value("size", 0);
            std::string processor = model.value("processor", "CPU");
            std::string expires = model.value("expires_at", "");

            // Extended metrics
            double vram_usage = model.value("vram_usage_percent", 0.0);
            double temperature = model.value("gpu_temperature", 0.0);
            uint64_t requests = model.value("request_count", 0);

            // Format size
            std::string size_str;
            if (size >= 1024ULL * 1024 * 1024) {
                size_str = std::to_string(size / (1024ULL * 1024 * 1024)) + " GB";
            } else if (size >= 1024ULL * 1024) {
                size_str = std::to_string(size / (1024ULL * 1024)) + " MB";
            } else {
                size_str = std::to_string(size) + " B";
            }

            // Format VRAM usage
            std::ostringstream vram_oss;
            vram_oss << std::fixed << std::setprecision(0) << vram_usage << "%";

            // Format temperature
            std::ostringstream temp_oss;
            if (temperature > 0) {
                temp_oss << std::fixed << std::setprecision(0) << temperature << "°C";
            } else {
                temp_oss << "-";
            }

            // Format requests
            std::string req_str = std::to_string(requests);

            std::cout << std::left
                      << std::setw(40) << name
                      << std::setw(15) << id
                      << std::setw(12) << size_str
                      << std::setw(15) << processor
                      << std::setw(10) << vram_oss.str()
                      << std::setw(8) << temp_oss.str()
                      << std::setw(12) << req_str
                      << std::setw(15) << expires
                      << std::endl;
        }
    }

    return 0;
}

}  // namespace commands
}  // namespace cli
}  // namespace xllm
