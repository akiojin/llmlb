#pragma once

#include <string>
#include <vector>

#include "core/engine_types.h"

namespace xllm {

std::string get_default_lora_dir();

std::vector<LoraRequest> resolve_lora_requests(
    const std::vector<LoraRequest>& requests,
    const std::string& base_dir,
    std::string& error);

}  // namespace xllm
