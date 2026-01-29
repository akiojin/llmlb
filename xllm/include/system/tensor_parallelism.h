#pragma once

#include <cstddef>
#include <cstdint>
#include <vector>

#include "system/gpu_detector.h"

namespace xllm {

// T203: Tensor Parallelism自動化（VRAM比例のtensor_split決定）
std::vector<float> computeTensorSplitRatios(const std::vector<GpuDevice>& devices);

// T211: GPU Offload自動化（VRAMに応じたn_gpu_layers算出）
size_t computeGpuLayersForOffload(size_t total_layers,
                                  uint64_t model_vram_bytes,
                                  uint64_t available_vram_bytes);

}  // namespace xllm
