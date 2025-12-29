#pragma once

#include <string>
#include <optional>
#include <nlohmann/json.hpp>

namespace llm_node {

struct ModelDescriptor {
    std::string name;
    std::string runtime;
    std::string format;
    std::string primary_path;
    std::string model_dir;
    std::optional<nlohmann::json> metadata;
};

}  // namespace llm_node
