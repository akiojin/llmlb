#include "core/engine_host.h"

#include "core/engine_registry.h"

#include <algorithm>
#include <fstream>
#include <sstream>
#include <cctype>
#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>

#ifdef _WIN32
#include <windows.h>
#else
#include <dlfcn.h>
#endif

namespace llm_node {

// T183: プラグインログレベルを文字列に変換
const char* pluginLogLevelToString(PluginLogLevel level) {
    switch (level) {
        case PluginLogLevel::kTrace: return "trace";
        case PluginLogLevel::kDebug: return "debug";
        case PluginLogLevel::kInfo:  return "info";
        case PluginLogLevel::kWarn:  return "warn";
        case PluginLogLevel::kError: return "error";
        default: return "unknown";
    }
}

// T183: デフォルトのプラグインログハンドラ
void defaultPluginLogHandler(void* /*ctx*/, const char* plugin_id, int level, const char* message) {
    if (!plugin_id || !message) return;

    const auto log_level = static_cast<PluginLogLevel>(level);

    // spdlogにプラグインIDプレフィックス付きでログ出力
    // タイムスタンプはspdlogが自動付与
    switch (log_level) {
        case PluginLogLevel::kTrace:
            spdlog::trace("[plugin:{}] {}", plugin_id, message);
            break;
        case PluginLogLevel::kDebug:
            spdlog::debug("[plugin:{}] {}", plugin_id, message);
            break;
        case PluginLogLevel::kInfo:
            spdlog::info("[plugin:{}] {}", plugin_id, message);
            break;
        case PluginLogLevel::kWarn:
            spdlog::warn("[plugin:{}] {}", plugin_id, message);
            break;
        case PluginLogLevel::kError:
            spdlog::error("[plugin:{}] {}", plugin_id, message);
            break;
        default:
            spdlog::info("[plugin:{}] {}", plugin_id, message);
            break;
    }
}

namespace {

struct LibraryHandle {
#ifdef _WIN32
    HMODULE handle{nullptr};
#else
    void* handle{nullptr};
#endif

    explicit operator bool() const { return handle != nullptr; }
};

LibraryHandle openLibrary(const std::filesystem::path& path, std::string& error) {
    LibraryHandle lib;
#ifdef _WIN32
    lib.handle = LoadLibraryA(path.string().c_str());
    if (!lib.handle) {
        error = "failed to load library: " + path.string();
    }
#else
    lib.handle = dlopen(path.string().c_str(), RTLD_NOW);
    if (!lib.handle) {
        const char* dl_error = dlerror();
        error = dl_error ? dl_error : ("failed to load library: " + path.string());
    }
#endif
    return lib;
}

void closeLibrary(LibraryHandle& lib) {
    if (!lib) return;
#ifdef _WIN32
    FreeLibrary(lib.handle);
#else
    dlclose(lib.handle);
#endif
    lib.handle = nullptr;
}

void* loadSymbol(LibraryHandle& lib, const char* symbol, std::string& error) {
    if (!lib) {
        error = "library handle not initialized";
        return nullptr;
    }
#ifdef _WIN32
    void* fn = reinterpret_cast<void*>(GetProcAddress(lib.handle, symbol));
    if (!fn) {
        error = std::string("missing symbol: ") + symbol;
    }
    return fn;
#else
    dlerror();
    void* fn = dlsym(lib.handle, symbol);
    const char* dl_error = dlerror();
    if (dl_error) {
        error = dl_error;
        return nullptr;
    }
    return fn;
#endif
}

bool parseStringField(const nlohmann::json& j,
                      const char* key,
                      std::string& out,
                      std::string& error) {
    if (!j.contains(key)) {
        error = std::string(key) + " is required";
        return false;
    }
    if (!j[key].is_string()) {
        error = std::string(key) + " must be a string";
        return false;
    }
    out = j[key].get<std::string>();
    return true;
}

bool parseStringArray(const nlohmann::json& j,
                      const char* key,
                      std::vector<std::string>& out,
                      std::string& error) {
    if (!j.contains(key)) {
        error = std::string(key) + " is required";
        return false;
    }
    if (!j[key].is_array()) {
        error = std::string(key) + " must be an array";
        return false;
    }
    out.clear();
    for (const auto& item : j[key]) {
        if (!item.is_string()) {
            error = std::string(key) + " must be an array of strings";
            return false;
        }
        out.push_back(item.get<std::string>());
    }
    return true;
}

std::string platformLibraryName(const std::string& base_name) {
#ifdef _WIN32
    return base_name + ".dll";
#else
    std::string name = base_name;
    if (name.rfind("lib", 0) != 0) {
        name = "lib" + name;
    }
#ifdef __APPLE__
    return name + ".dylib";
#else
    return name + ".so";
#endif
#endif
}

std::filesystem::path resolveLibraryPath(const std::filesystem::path& manifest_dir,
                                         const std::string& library) {
    std::filesystem::path lib_path = library;
    if (lib_path.is_relative()) {
        lib_path = manifest_dir / lib_path;
    }
    if (!lib_path.has_extension()) {
        auto filename = platformLibraryName(lib_path.filename().string());
        lib_path = lib_path.parent_path() / filename;
    }
    return lib_path;
}

std::string toLower(std::string value) {
    for (auto& ch : value) {
        ch = static_cast<char>(std::tolower(static_cast<unsigned char>(ch)));
    }
    return value;
}

std::vector<std::string> supportedGpuTargets() {
    std::vector<std::string> targets;
#ifdef USE_METAL
    targets.push_back("metal");
#endif
#ifdef _WIN32
    targets.push_back("directml");
#endif
#ifdef USE_CUDA
    targets.push_back("cuda");
#endif
#ifdef USE_ROCM
    targets.push_back("rocm");
#endif
    return targets;
}

bool isGpuTargetCompatible(const std::vector<std::string>& gpu_targets) {
    if (gpu_targets.empty()) return true;
    const auto supported = supportedGpuTargets();
    if (supported.empty()) return false;

    for (const auto& target : gpu_targets) {
        const auto needle = toLower(target);
        for (const auto& candidate : supported) {
            if (needle == candidate) {
                return true;
            }
        }
    }
    return false;
}

struct LoadedPluginState {
    std::string engine_id;
    std::filesystem::path library_path;
    LibraryHandle library;
    EngineRegistry::EngineHandle engine;
    EngineRegistration registration;
};

bool preparePlugin(const std::filesystem::path& manifest_path,
                   const EngineHostContext& context,
                   LoadedPluginState& state,
                   bool& skipped,
                   std::string& error,
                   const EngineHost& host) {
    skipped = false;
    state = {};

    EnginePluginManifest manifest;
    if (!host.loadManifest(manifest_path, manifest, error)) return false;

    if (!isGpuTargetCompatible(manifest.gpu_targets)) {
        skipped = true;
        error.clear();
        return true;
    }

    auto lib_path = resolveLibraryPath(manifest_path.parent_path(), manifest.library);
    auto lib = openLibrary(lib_path, error);
    if (!lib) return false;

    auto create_fn = reinterpret_cast<llm_node_create_engine_fn>(
        loadSymbol(lib, "llm_node_create_engine", error));
    if (!create_fn) {
        closeLibrary(lib);
        return false;
    }
    auto destroy_fn = reinterpret_cast<llm_node_destroy_engine_fn>(
        loadSymbol(lib, "llm_node_destroy_engine", error));
    if (!destroy_fn) {
        closeLibrary(lib);
        return false;
    }

    Engine* engine = create_fn(&context);
    if (!engine) {
        error = "engine factory returned null";
        closeLibrary(lib);
        return false;
    }

    if (std::find(manifest.runtimes.begin(), manifest.runtimes.end(), engine->runtime()) ==
        manifest.runtimes.end()) {
        error = "engine runtime not declared in manifest";
        destroy_fn(engine);
        closeLibrary(lib);
        return false;
    }

    EngineDeleter deleter;
    deleter.destroy = destroy_fn;
    EngineRegistry::EngineHandle handle(engine, deleter);

    EngineRegistration registration;
    registration.engine_id = manifest.engine_id;
    registration.engine_version = manifest.engine_version;
    registration.formats = manifest.formats;
    registration.architectures = manifest.architectures;
    registration.capabilities = manifest.capabilities;

    state.engine_id = manifest.engine_id;
    state.library_path = lib_path;
    state.library = lib;
    state.engine = std::move(handle);
    state.registration = std::move(registration);
    return true;
}

}  // namespace

EngineHost::~EngineHost() {
    for (auto& pending : pending_) {
        pending.engine.reset();
        LibraryHandle handle;
#ifdef _WIN32
        handle.handle = reinterpret_cast<HMODULE>(pending.handle);
#else
        handle.handle = pending.handle;
#endif
        closeLibrary(handle);
        pending.handle = nullptr;
    }
    for (auto& plugin : plugins_) {
        LibraryHandle handle;
#ifdef _WIN32
        handle.handle = reinterpret_cast<HMODULE>(plugin.handle);
#else
        handle.handle = plugin.handle;
#endif
        closeLibrary(handle);
        plugin.handle = nullptr;
    }
}

bool EngineHost::validateManifest(const EnginePluginManifest& manifest,
                                  std::string& error) const {
    error.clear();

    if (manifest.engine_id.empty()) {
        error = "engine_id is required";
        return false;
    }
    if (manifest.engine_version.empty()) {
        error = "engine_version is required";
        return false;
    }
    if (manifest.abi_version != kAbiVersion) {
        error = "abi_version mismatch";
        return false;
    }
    if (manifest.runtimes.empty()) {
        error = "runtimes is required";
        return false;
    }
    if (manifest.formats.empty()) {
        error = "formats is required";
        return false;
    }
    if (manifest.architectures.empty()) {
        error = "architectures is required";
        return false;
    }
    if (manifest.modalities.empty()) {
        error = "modalities is required";
        return false;
    }
    if (manifest.license.empty()) {
        error = "license is required";
        return false;
    }
    if (manifest.library.empty()) {
        error = "library is required";
        return false;
    }

    for (const auto& runtime : manifest.runtimes) {
        if (runtime.empty()) {
            error = "runtimes contains empty value";
            return false;
        }
    }
    for (const auto& format : manifest.formats) {
        if (format.empty()) {
            error = "formats contains empty value";
            return false;
        }
    }
    for (const auto& arch : manifest.architectures) {
        if (arch.empty()) {
            error = "architectures contains empty value";
            return false;
        }
    }
    for (const auto& modality : manifest.modalities) {
        if (modality.empty()) {
            error = "modalities contains empty value";
            return false;
        }
    }

    return true;
}

bool EngineHost::loadManifest(const std::filesystem::path& manifest_path,
                              EnginePluginManifest& manifest,
                              std::string& error) const {
    error.clear();
    if (!std::filesystem::exists(manifest_path)) {
        error = "manifest not found: " + manifest_path.string();
        return false;
    }

    nlohmann::json j;
    try {
        std::ifstream ifs(manifest_path);
        if (!ifs.is_open()) {
            error = "failed to open manifest: " + manifest_path.string();
            return false;
        }
        ifs >> j;
    } catch (const std::exception& ex) {
        error = std::string("invalid manifest JSON: ") + ex.what();
        return false;
    }

    if (!parseStringField(j, "engine_id", manifest.engine_id, error)) return false;
    if (!parseStringField(j, "engine_version", manifest.engine_version, error)) return false;
    if (!parseStringField(j, "license", manifest.license, error)) return false;
    if (!parseStringField(j, "library", manifest.library, error)) return false;

    if (!j.contains("abi_version") || !j["abi_version"].is_number_integer()) {
        error = "abi_version is required";
        return false;
    }
    manifest.abi_version = j["abi_version"].get<int>();

    if (!parseStringArray(j, "runtimes", manifest.runtimes, error)) return false;
    if (!parseStringArray(j, "formats", manifest.formats, error)) return false;
    if (!parseStringArray(j, "architectures", manifest.architectures, error)) return false;
    if (!parseStringArray(j, "modalities", manifest.modalities, error)) return false;

    if (!j.contains("supports_vision") || !j["supports_vision"].is_boolean()) {
        error = "supports_vision is required";
        return false;
    }
    manifest.supports_vision = j["supports_vision"].get<bool>();

    if (j.contains("architectures")) {
        if (!j["architectures"].is_array()) {
            error = "architectures must be an array";
            return false;
        }
        manifest.architectures.clear();
        for (const auto& item : j["architectures"]) {
            if (!item.is_string()) {
                error = "architectures must be an array of strings";
                return false;
            }
            manifest.architectures.push_back(item.get<std::string>());
        }
    }

    if (j.contains("capabilities")) {
        if (!j["capabilities"].is_array()) {
            error = "capabilities must be an array";
            return false;
        }
        manifest.capabilities.clear();
        for (const auto& item : j["capabilities"]) {
            if (!item.is_string()) {
                error = "capabilities must be an array of strings";
                return false;
            }
            manifest.capabilities.push_back(item.get<std::string>());
        }
    }

    if (j.contains("gpu_targets")) {
        if (!j["gpu_targets"].is_array()) {
            error = "gpu_targets must be an array";
            return false;
        }
        manifest.gpu_targets.clear();
        for (const auto& item : j["gpu_targets"]) {
            if (!item.is_string()) {
                error = "gpu_targets must be an array of strings";
                return false;
            }
            manifest.gpu_targets.push_back(item.get<std::string>());
        }
    }

    return validateManifest(manifest, error);
}

bool EngineHost::loadPlugin(const std::filesystem::path& manifest_path,
                            EngineRegistry& registry,
                            const EngineHostContext& context,
                            std::string& error) {
    LoadedPluginState state;
    bool skipped = false;
    if (!preparePlugin(manifest_path, context, state, skipped, error, *this)) return false;
    if (skipped) return true;

    std::string reg_error;
    if (!registry.registerEngine(std::move(state.engine), state.registration, &reg_error)) {
        error = reg_error;
        closeLibrary(state.library);
        return false;
    }

    LoadedPlugin loaded;
    loaded.engine_id = state.engine_id;
    loaded.library_path = state.library_path;
#ifdef _WIN32
    loaded.handle = reinterpret_cast<void*>(state.library.handle);
#else
    loaded.handle = state.library.handle;
#endif
    plugins_.push_back(std::move(loaded));
    return true;
}

bool EngineHost::loadPluginsFromDir(const std::filesystem::path& directory,
                                    EngineRegistry& registry,
                                    const EngineHostContext& context,
                                    std::string& error) {
    error.clear();
    if (directory.empty()) return true;

    std::error_code ec;
    if (!std::filesystem::exists(directory, ec)) {
        return true;
    }

    bool ok = true;
    for (const auto& entry : std::filesystem::directory_iterator(directory, ec)) {
        if (ec) break;
        std::filesystem::path manifest_path;
        if (entry.is_directory()) {
            manifest_path = entry.path() / "manifest.json";
        } else if (entry.is_regular_file() && entry.path().filename() == "manifest.json") {
            manifest_path = entry.path();
        } else {
            continue;
        }

        if (!std::filesystem::exists(manifest_path)) {
            continue;
        }

        std::string load_error;
        if (!loadPlugin(manifest_path, registry, context, load_error)) {
            ok = false;
            if (error.empty()) {
                error = load_error;
            }
        }
    }

    if (ec && error.empty()) {
        error = "failed to scan plugin directory: " + directory.string();
        return false;
    }

    return ok;
}

bool EngineHost::stagePlugin(const std::filesystem::path& manifest_path,
                             const EngineHostContext& context,
                             std::string& error) {
    LoadedPluginState state;
    bool skipped = false;
    if (!preparePlugin(manifest_path, context, state, skipped, error, *this)) return false;
    if (skipped) return true;

    auto pending_it = std::find_if(pending_.begin(), pending_.end(), [&](const PendingPlugin& entry) {
        return entry.engine_id == state.engine_id;
    });
    if (pending_it != pending_.end()) {
        pending_it->engine.reset();
        LibraryHandle handle;
#ifdef _WIN32
        handle.handle = reinterpret_cast<HMODULE>(pending_it->handle);
#else
        handle.handle = pending_it->handle;
#endif
        closeLibrary(handle);
        pending_.erase(pending_it);
    }

    PendingPlugin pending;
    pending.engine_id = state.engine_id;
    pending.library_path = state.library_path;
#ifdef _WIN32
    pending.handle = reinterpret_cast<void*>(state.library.handle);
#else
    pending.handle = state.library.handle;
#endif
    pending.engine = std::move(state.engine);
    pending.registration = std::move(state.registration);
    pending_.push_back(std::move(pending));
    return true;
}

bool EngineHost::stagePluginsFromDir(const std::filesystem::path& directory,
                                     const EngineHostContext& context,
                                     std::string& error) {
    error.clear();
    if (directory.empty()) return true;

    std::error_code ec;
    if (!std::filesystem::exists(directory, ec)) {
        return true;
    }

    bool ok = true;
    for (const auto& entry : std::filesystem::directory_iterator(directory, ec)) {
        if (ec) break;
        std::filesystem::path manifest_path;
        if (entry.is_directory()) {
            manifest_path = entry.path() / "manifest.json";
        } else if (entry.is_regular_file() && entry.path().filename() == "manifest.json") {
            manifest_path = entry.path();
        } else {
            continue;
        }

        if (!std::filesystem::exists(manifest_path)) {
            continue;
        }

        std::string load_error;
        if (!stagePlugin(manifest_path, context, load_error)) {
            ok = false;
            if (error.empty()) {
                error = load_error;
            }
        }
    }

    if (ec && error.empty()) {
        error = "failed to scan plugin directory: " + directory.string();
        return false;
    }

    return ok;
}

bool EngineHost::applyPendingPlugins(EngineRegistry& registry, std::string& error) {
    error.clear();
    bool ok = true;
    for (auto& pending : pending_) {
        std::string reg_error;
        EngineRegistry::EngineHandle replaced;
        if (!registry.replaceEngine(std::move(pending.engine), pending.registration, &replaced, &reg_error)) {
            ok = false;
            if (error.empty()) {
                error = reg_error;
            }
            LibraryHandle handle;
#ifdef _WIN32
            handle.handle = reinterpret_cast<HMODULE>(pending.handle);
#else
            handle.handle = pending.handle;
#endif
            closeLibrary(handle);
            pending.handle = nullptr;
            continue;
        }

        LibraryHandle old_handle;
        bool had_old_handle = false;
        auto existing_it = std::find_if(plugins_.begin(), plugins_.end(), [&](const LoadedPlugin& entry) {
            return entry.engine_id == pending.engine_id;
        });
        if (existing_it != plugins_.end()) {
#ifdef _WIN32
            old_handle.handle = reinterpret_cast<HMODULE>(existing_it->handle);
#else
            old_handle.handle = existing_it->handle;
#endif
            had_old_handle = true;
            plugins_.erase(existing_it);
        }

        replaced.reset();
        if (had_old_handle) {
            closeLibrary(old_handle);
        }

        LoadedPlugin loaded;
        loaded.engine_id = pending.engine_id;
        loaded.library_path = pending.library_path;
        loaded.handle = pending.handle;
        plugins_.push_back(std::move(loaded));
    }

    pending_.clear();
    return ok;
}

}  // namespace llm_node
