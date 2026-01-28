#include "cli/commands.h"

#include <filesystem>
#include <fstream>
#include <iostream>

#include "cli/cli_client.h"

namespace fs = std::filesystem;

namespace xllm::cli::commands {

int exportModel(const ExportOptions& options) {
    CliClient client;
    auto resp = client.showModel(options.model);
    if (!resp.ok()) {
        std::cerr << "Error: " << resp.error_message << "\n";
        return resp.error == CliError::ConnectionError ? 2 : 1;
    }

    fs::path out_path = options.output;
    if (out_path == "-") {
        std::cout << resp.data->dump(2) << "\n";
        return 0;
    }

    fs::create_directories(out_path.parent_path());
    std::ofstream ofs(out_path);
    if (!ofs.is_open()) {
        std::cerr << "Error: failed to open output file: " << out_path.string() << "\n";
        return 1;
    }
    ofs << resp.data->dump(2);
    std::cout << "Exported to: " << out_path.string() << "\n";
    return 0;
}

}  // namespace xllm::cli::commands
