#pragma once

#include <string>
#include <string_view>

namespace llm_node {

// Returns a valid UTF-8 string. Invalid sequences are replaced with U+FFFD.
std::string sanitize_utf8_lossy(std::string_view input);

}  // namespace llm_node

