#include "core/engine_host.h"

#include "core/engine_registry.h"

#include <algorithm>
#include <fstream>
#include <sstream>
#include <nlohmann/json.hpp>

#ifdef _WIN32
#include <windows.h>
#else
#include <dlfcn.h>
#endif

namespace llm_node {

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

}  // namespace

EngineHost::~EngineHost() {
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
    if (!parseStringField(j, "library", manifest.library, error)) return false;

    if (!j.contains("abi_version") || !j["abi_version"].is_number_integer()) {
        error = "abi_version is required";
        return false;
    }
    manifest.abi_version = j["abi_version"].get<int>();

    if (!parseStringArray(j, "runtimes", manifest.runtimes, error)) return false;
    if (!parseStringArray(j, "formats", manifest.formats, error)) return false;

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
    EnginePluginManifest manifest;
    if (!loadManifest(manifest_path, manifest, error)) return false;

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
    registry.registerEngine(std::move(handle));

    LoadedPlugin loaded;
    loaded.engine_id = manifest.engine_id;
    loaded.library_path = lib_path;
#ifdef _WIN32
    loaded.handle = reinterpret_cast<void*>(lib.handle);
#else
    loaded.handle = lib.handle;
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

}  // namespace llm_node
