#include "core/engine_registry.h"

#include <algorithm>
#include <cctype>
#include <limits>
#include <optional>
#include <sstream>
#include <spdlog/spdlog.h>

namespace allm {

namespace {
std::string normalize_architecture_name(const std::string& value) {
    std::string lower = value;
    std::transform(lower.begin(), lower.end(), lower.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    std::string compact;
    compact.reserve(lower.size());
    for (char c : lower) {
        if (std::isalnum(static_cast<unsigned char>(c))) {
            compact.push_back(c);
        }
    }
    if (compact.find("mistral") != std::string::npos) return "mistral";
    if (compact.find("gemma") != std::string::npos) return "gemma";
    if (compact.find("llama") != std::string::npos) return "llama";
    if (compact.find("qwen") != std::string::npos) return "qwen";
    if (compact.find("glm") != std::string::npos) return "glm";
    return compact.empty() ? lower : compact;
}

bool architectures_compatible(const std::vector<std::string>& supported,
                              const std::vector<std::string>& requested) {
    if (requested.empty()) return true;
    if (supported.empty()) return true;

    std::vector<std::string> normalized_supported;
    normalized_supported.reserve(supported.size());
    for (const auto& entry : supported) {
        normalized_supported.push_back(normalize_architecture_name(entry));
    }

    for (const auto& req : requested) {
        const auto needle = normalize_architecture_name(req);
        for (const auto& candidate : normalized_supported) {
            if (needle == candidate) {
                return true;
            }
        }
    }
    return false;
}
}  // namespace

std::vector<const EngineRegistry::EngineEntry*> EngineRegistry::filterCandidates(
    const std::vector<EngineRegistry::EngineEntry>& entries,
    const ModelDescriptor& descriptor,
    const std::string& capability) {
    std::vector<const EngineRegistry::EngineEntry*> candidates;
    candidates.reserve(entries.size());

    for (const auto& entry : entries) {
        if (!descriptor.format.empty() && !entry.formats.empty()) {
            if (std::find(entry.formats.begin(), entry.formats.end(), descriptor.format) == entry.formats.end()) {
                continue;
            }
        }

        if (!capability.empty() && !entry.capabilities.empty()) {
            if (std::find(entry.capabilities.begin(), entry.capabilities.end(), capability) == entry.capabilities.end()) {
                continue;
            }
        }

        candidates.push_back(&entry);
    }

    return candidates;
}

std::string normalize_architecture(std::string value) {
    std::string out;
    out.reserve(value.size());
    for (char c : value) {
        if (std::isalnum(static_cast<unsigned char>(c))) {
            out.push_back(static_cast<char>(std::tolower(static_cast<unsigned char>(c))));
        }
    }
    // Normalize architecture family names (e.g., qwen2 -> qwen, llama3 -> llama)
    if (out.find("qwen") != std::string::npos) return "qwen";
    if (out.find("llama") != std::string::npos) return "llama";
    if (out.find("mistral") != std::string::npos) return "mistral";
    if (out.find("gemma") != std::string::npos) return "gemma";
    if (out.find("phi") != std::string::npos) return "phi";
    if (out.find("nemotron") != std::string::npos) return "nemotron";
    if (out.find("deepseek") != std::string::npos) return "deepseek";
    if (out.find("gptoss") != std::string::npos) return "gptoss";
    if (out.find("glm") != std::string::npos) return "glm";
    if (out.find("granite") != std::string::npos) return "granite";
    if (out.find("smollm") != std::string::npos) return "smollm";
    if (out.find("kimi") != std::string::npos) return "kimi";
    if (out.find("moondream") != std::string::npos) return "moondream";
    if (out.find("snowflake") != std::string::npos) return "snowflake";
    if (out.find("nomic") != std::string::npos) return "nomic";
    if (out.find("mxbai") != std::string::npos) return "mxbai";
    if (out.find("minilm") != std::string::npos) return "minilm";
    if (out.find("devstral") != std::string::npos) return "devstral";
    if (out.find("magistral") != std::string::npos) return "magistral";
    return out;
}

bool has_architecture_match(const std::vector<std::string>& supported,
                            const std::vector<std::string>& requested) {
    if (supported.empty() || requested.empty()) return true;
    std::vector<std::string> normalized_supported;
    normalized_supported.reserve(supported.size());
    for (const auto& s : supported) {
        normalized_supported.push_back(normalize_architecture(s));
    }
    for (const auto& r : requested) {
        const auto nr = normalize_architecture(r);
        if (nr.empty()) continue;
        if (std::find(normalized_supported.begin(), normalized_supported.end(), nr) != normalized_supported.end()) {
            return true;
        }
    }
    return false;
}

std::string join_architectures(const std::vector<std::string>& values) {
    std::ostringstream oss;
    oss << "[";
    for (size_t i = 0; i < values.size(); ++i) {
        if (i > 0) oss << ", ";
        oss << "'" << values[i] << "'";
    }
    oss << "]";
    return oss.str();
}

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
    entry.architectures = registration.architectures;
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

bool EngineRegistry::replaceEngine(EngineHandle engine,
                                   const EngineRegistration& registration,
                                   EngineHandle* replaced,
                                   std::string* error) {
    if (!engine) return false;

    const std::string runtime = engine->runtime();
    const std::string engine_id = registration.engine_id.empty() ? runtime : registration.engine_id;
    const std::string engine_version = registration.engine_version.empty() ? "builtin" : registration.engine_version;

    auto existing = engine_ids_.find(engine_id);
    if (existing == engine_ids_.end()) {
        if (replaced) {
            replaced->reset();
        }
        return registerEngine(std::move(engine), registration, error);
    }

    if (existing->second != runtime) {
        if (error) {
            *error = "engine_id already registered for a different runtime: " + engine_id;
        }
        return false;
    }

    auto it = engines_.find(runtime);
    if (it == engines_.end()) {
        if (error) {
            *error = "engine runtime not found for replacement: " + runtime;
        }
        return false;
    }

    auto& entries = it->second;
    auto entry_it = std::find_if(entries.begin(), entries.end(), [&](const EngineEntry& entry) {
        return entry.engine_id == engine_id;
    });
    if (entry_it == entries.end()) {
        if (error) {
            *error = "engine_id not found for replacement: " + engine_id;
        }
        return false;
    }

    if (replaced) {
        *replaced = std::move(entry_it->engine);
    }

    entry_it->engine = std::move(engine);
    entry_it->engine_version = engine_version;
    entry_it->formats = registration.formats;
    entry_it->architectures = registration.architectures;
    entry_it->capabilities = registration.capabilities;
    return true;
}

Engine* EngineRegistry::resolve(const std::string& runtime) const {
    auto it = engines_.find(runtime);
    if (it == engines_.end()) return nullptr;
    const auto& entries = it->second;
    if (entries.empty()) return nullptr;
    return entries.front().engine.get();
}

Engine* EngineRegistry::resolve(const ModelDescriptor& descriptor) const {
    return resolve(descriptor, "");
}

Engine* EngineRegistry::resolve(const ModelDescriptor& descriptor, const std::string& capability) const {
    return resolve(descriptor, capability, nullptr);
}

Engine* EngineRegistry::resolve(const ModelDescriptor& descriptor,
                                const std::string& capability,
                                std::string* error) const {
    auto it = engines_.find(descriptor.runtime);
    if (it == engines_.end()) {
        if (error) *error = "No engine registered for runtime: " + descriptor.runtime;
        return nullptr;
    }
    const auto& entries = it->second;
    if (entries.empty()) {
        if (error) *error = "No engine registered for runtime: " + descriptor.runtime;
        return nullptr;
    }

    auto candidates = filterCandidates(entries, descriptor, capability);
    if (candidates.empty()) {
        if (error) *error = "No engine registered for runtime: " + descriptor.runtime;
        return nullptr;
    }

    if (!descriptor.architectures.empty()) {
        std::vector<const EngineEntry*> arch_candidates;
        arch_candidates.reserve(candidates.size());
        for (const auto* entry : candidates) {
            if (has_architecture_match(entry->architectures, descriptor.architectures)) {
                arch_candidates.push_back(entry);
            }
        }
        if (arch_candidates.empty()) {
            if (error) {
                std::vector<std::string> supported;
                for (const auto* entry : candidates) {
                    supported.insert(supported.end(),
                                     entry->architectures.begin(),
                                     entry->architectures.end());
                }
                const std::string requested = descriptor.architectures.front();
                const std::string engine_id = candidates.front()->engine_id;
                *error = "Model architecture '" + requested +
                         "' is not supported by engine '" + engine_id +
                         "'. Supported: " + join_architectures(supported);
            }
            return nullptr;
        }
        candidates = std::move(arch_candidates);
    }

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

bool EngineRegistry::hasRuntime(const std::string& runtime) const {
    auto it = engines_.find(runtime);
    if (it == engines_.end()) return false;
    return !it->second.empty();
}

bool EngineRegistry::supportsArchitecture(const std::string& runtime,
                                          const std::vector<std::string>& architectures) const {
    auto it = engines_.find(runtime);
    if (it == engines_.end()) return false;
    const auto& entries = it->second;
    if (entries.empty()) return false;
    if (architectures.empty()) return true;
    for (const auto& entry : entries) {
        if (architectures_compatible(entry.architectures, architectures)) {
            return true;
        }
    }
    return false;
}

size_t EngineRegistry::engineIdCount() const {
    return engine_ids_.size();
}

std::string EngineRegistry::engineIdFor(const Engine* engine) const {
    if (!engine) return "";
    for (const auto& pair : engines_) {
        for (const auto& entry : pair.second) {
            if (entry.engine.get() == engine) {
                return entry.engine_id;
            }
        }
    }
    return "";
}

std::vector<std::string> EngineRegistry::getRegisteredRuntimes() const {
    std::vector<std::string> runtimes;
    runtimes.reserve(engines_.size());
    for (const auto& pair : engines_) {
        runtimes.push_back(pair.first);
    }
    return runtimes;
}

}  // namespace allm
