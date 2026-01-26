#pragma once

#include <optional>
#include <string>
#include <vector>
#include <nlohmann/json.hpp>

namespace xllm {

struct ModelDescriptor {
    std::string name;
    std::string runtime;
    std::string format;
    std::string primary_path;
    std::string model_dir;
    std::optional<nlohmann::json> metadata;
    std::vector<std::string> architectures;
    std::vector<std::string> capabilities;
};

}  // namespace xllm
