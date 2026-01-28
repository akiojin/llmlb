#include "cli/commands.h"

#include <filesystem>
#include <fstream>
#include <iostream>

#include "models/modelfile.h"

namespace fs = std::filesystem;

namespace xllm::cli::commands {

int importModel(const ImportOptions& options) {
    fs::path source = options.file;
    if (!fs::exists(source)) {
        std::cerr << "Error: file not found: " << source.string() << "\n";
        return 1;
    }

    std::ifstream ifs(source);
    if (!ifs.is_open()) {
        std::cerr << "Error: failed to open file: " << source.string() << "\n";
        return 1;
    }

    std::string content((std::istreambuf_iterator<char>(ifs)), std::istreambuf_iterator<char>());

    fs::path dest = Modelfile::pathForModel(options.model);
    fs::create_directories(dest.parent_path());

    std::ofstream ofs(dest);
    if (!ofs.is_open()) {
        std::cerr << "Error: failed to write Modelfile: " << dest.string() << "\n";
        return 1;
    }
    ofs << content;
    std::cout << "Imported Modelfile to: " << dest.string() << "\n";
    return 0;
}

}  // namespace xllm::cli::commands
