#include <algorithm>
#include <cstddef>
#include <cstdlib>
#include <iostream>
#include <map>
#include <string>
#include <vector>

#define SAFETENSORS_CPP_IMPLEMENTATION
#include "safetensors.hh"

namespace {
const char kKnownTensor[] = "backbone.layers.1.mixer.experts.0.down_proj.weight";

std::string dtype_name(safetensors::dtype dtype) {
  switch (dtype) {
    case safetensors::kBOOL:
      return "BOOL";
    case safetensors::kUINT8:
      return "UINT8";
    case safetensors::kINT8:
      return "INT8";
    case safetensors::kINT16:
      return "INT16";
    case safetensors::kUINT16:
      return "UINT16";
    case safetensors::kFLOAT16:
      return "F16";
    case safetensors::kBFLOAT16:
      return "BF16";
    case safetensors::kINT32:
      return "INT32";
    case safetensors::kUINT32:
      return "UINT32";
    case safetensors::kFLOAT32:
      return "F32";
    case safetensors::kFLOAT64:
      return "F64";
    case safetensors::kINT64:
      return "INT64";
    case safetensors::kUINT64:
      return "UINT64";
  }
  return "UNKNOWN";
}

std::string shape_to_string(const std::vector<size_t> &shape) {
  std::string out = "[";
  for (size_t i = 0; i < shape.size(); i++) {
    out += std::to_string(shape[i]);
    if (i + 1 < shape.size()) {
      out += ", ";
    }
  }
  out += "]";
  return out;
}

void print_usage(const char *argv0) {
  std::cerr << "Usage: " << argv0
            << " <safetensors_file> [--limit N] [--match STR]" << "\n";
}
}  // namespace

int main(int argc, char **argv) {
  if (argc < 2) {
    print_usage(argv[0]);
    return 2;
  }

  std::string filename;
  size_t limit = 20;
  std::string match;

  filename = argv[1];
  for (int i = 2; i < argc; i++) {
    std::string arg = argv[i];
    if (arg == "--limit") {
      if (i + 1 >= argc) {
        std::cerr << "--limit requires a value" << "\n";
        return 2;
      }
      limit = static_cast<size_t>(std::stoul(argv[++i]));
    } else if (arg == "--match") {
      if (i + 1 >= argc) {
        std::cerr << "--match requires a value" << "\n";
        return 2;
      }
      match = argv[++i];
    } else {
      std::cerr << "Unknown arg: " << arg << "\n";
      print_usage(argv[0]);
      return 2;
    }
  }

  safetensors::safetensors_t st;
  std::string warn;
  std::string err;
  bool ok = safetensors::mmap_from_file(filename, &st, &warn, &err);
  if (!warn.empty()) {
    std::cout << "WARN: " << warn << "\n";
  }
  if (!ok) {
    std::cerr << "Failed to load: " << filename << "\n";
    std::cerr << err << "\n";
    return 1;
  }

  if (!safetensors::validate_data_offsets(st, err)) {
    std::cerr << "Invalid data offsets: " << err << "\n";
    return 1;
  }

  const std::vector<std::string> &keys = st.tensors.keys();
  std::map<safetensors::dtype, size_t> dtype_counts;
  size_t experts_count = 0;
  bool has_known_tensor = false;
  size_t matched = 0;

  size_t printed = 0;
  for (const auto &name : keys) {
    safetensors::tensor_t tensor;
    if (!st.tensors.at(name, &tensor)) {
      continue;
    }

    dtype_counts[tensor.dtype] += 1;

    if (name.find("experts") != std::string::npos) {
      experts_count += 1;
    }

    if (name == kKnownTensor) {
      has_known_tensor = true;
    }

    bool should_print = false;
    if (match.empty()) {
      should_print = printed < limit;
    } else if (name.find(match) != std::string::npos) {
      should_print = printed < limit;
      matched += 1;
    }

    if (should_print) {
      std::cout << name << " | " << dtype_name(tensor.dtype) << " | "
                << shape_to_string(tensor.shape) << "\n";
      printed += 1;
    }
  }

  std::cout << "\nSummary\n";
  std::cout << "Total tensors: " << keys.size() << "\n";
  std::cout << "Experts tensors: " << experts_count << "\n";
  std::cout << "Contains known failing tensor: "
            << (has_known_tensor ? "yes" : "no") << "\n";

  if (!match.empty()) {
    std::cout << "Matched ('" << match << "') tensors: " << matched << "\n";
  }

  std::cout << "Dtype counts:\n";
  for (const auto &entry : dtype_counts) {
    std::cout << "  " << dtype_name(entry.first) << ": " << entry.second << "\n";
  }

  return 0;
}
