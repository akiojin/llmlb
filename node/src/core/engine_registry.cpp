#include "core/engine_registry.h"

#include <algorithm>
#include <limits>
#include <optional>
#include <spdlog/spdlog.h>

namespace llm_node {

bool EngineRegistry::registerEngine(std::unique_ptr<Engine> engine,
                                    const EngineRegistration& registration,
                                    std::string* error) {
    if (!engine) return false;
    EngineHandle handle(engine.release(), EngineDeleter{});
    return registerEngine(std::move(handle), registration, error);
}

bool EngineRegistry::registerEngine(EngineHandle engine,
                                    const EngineRegistration& registration,
                                    std::string* error) {
    if (!engine) return false;

    const std::string runtime = engine->runtime();
    const std::string engine_id = registration.engine_id.empty() ? runtime : registration.engine_id;
    const std::string engine_version = registration.engine_version.empty() ? "builtin" : registration.engine_version;

    if (engine_ids_.find(engine_id) != engine_ids_.end()) {
        if (error) {
            *error = "engine_id already registered: " + engine_id;
        }
        return false;
    }

    engine_ids_.emplace(engine_id, runtime);
    EngineEntry entry;
    entry.engine_id = engine_id;
    entry.engine_version = engine_version;
    entry.formats = registration.formats;
    entry.capabilities = registration.capabilities;
    entry.engine = std::move(engine);
    engines_[runtime].push_back(std::move(entry));
    return true;
}

void EngineRegistry::registerEngine(std::unique_ptr<Engine> engine) {
    EngineRegistration reg;
    std::string error;
    if (!registerEngine(std::move(engine), reg, &error)) {
        if (!error.empty()) {
            spdlog::warn("EngineRegistry: {}", error);
        }
    }
}

void EngineRegistry::registerEngine(EngineHandle engine) {
    EngineRegistration reg;
    std::string error;
    if (!registerEngine(std::move(engine), reg, &error)) {
        if (!error.empty()) {
            spdlog::warn("EngineRegistry: {}", error);
        }
    }
}

Engine* EngineRegistry::resolve(const std::string& runtime) const {
    auto it = engines_.find(runtime);
    if (it == engines_.end()) return nullptr;
    const auto& entries = it->second;
    if (entries.empty()) return nullptr;
    return entries.front().engine.get();
}

Engine* EngineRegistry::resolve(const ModelDescriptor& descriptor) const {
    auto it = engines_.find(descriptor.runtime);
    if (it == engines_.end()) return nullptr;
    const auto& entries = it->second;
    if (entries.empty()) return nullptr;
    std::vector<const EngineEntry*> candidates;
    candidates.reserve(entries.size());
    for (const auto& entry : entries) {
        if (descriptor.format.empty() || entry.formats.empty()) {
            candidates.push_back(&entry);
            continue;
        }
        if (std::find(entry.formats.begin(), entry.formats.end(), descriptor.format) != entry.formats.end()) {
            candidates.push_back(&entry);
        }
    }

    if (candidates.empty()) return nullptr;
    if (candidates.size() == 1) return candidates.front()->engine.get();

    std::optional<std::string> preferred;
    if (descriptor.metadata.has_value()) {
        const auto& meta = *descriptor.metadata;
        if (meta.contains("benchmarks") && meta["benchmarks"].is_object()) {
            const auto& bench = meta["benchmarks"];
            if (bench.contains("preferred_engine_id") && bench["preferred_engine_id"].is_string()) {
                preferred = bench["preferred_engine_id"].get<std::string>();
            } else if (bench.contains("engine_scores") && bench["engine_scores"].is_object()) {
                double best_score = -std::numeric_limits<double>::infinity();
                for (auto it = bench["engine_scores"].begin(); it != bench["engine_scores"].end(); ++it) {
                    if (!it.value().is_number()) continue;
                    const auto engine_id = it.key();
                    const auto score = it.value().get<double>();
                    const bool exists = std::any_of(
                        candidates.begin(),
                        candidates.end(),
                        [&](const auto* entry) {
                            return entry->engine_id == engine_id;
                        });
                    if (!exists) continue;
                    if (score > best_score) {
                        best_score = score;
                        preferred = engine_id;
                    }
                }
            }
        }
    }

    if (preferred.has_value()) {
        for (const auto* entry : candidates) {
            if (entry->engine_id == *preferred) {
                return entry->engine.get();
            }
        }
        spdlog::warn("EngineRegistry: preferred engine_id not found for runtime {}", descriptor.runtime);
    } else {
        spdlog::warn("EngineRegistry: no benchmark metadata for runtime {}, using first engine",
                     descriptor.runtime);
    }

    return candidates.front()->engine.get();
}

Engine* EngineRegistry::resolve(const ModelDescriptor& descriptor, const std::string& capability) const {
    (void)capability;
    return resolve(descriptor);
}

}  // namespace llm_node
