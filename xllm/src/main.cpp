#include <iostream>
#include <memory>
#include <signal.h>
#include <atomic>
#include <thread>
#include <chrono>
#include <string>
#include <vector>
#include <filesystem>
#include <optional>
#include <algorithm>
#include <cctype>
#include <nlohmann/json.hpp>

#include "system/gpu_detector.h"
#include "system/resource_monitor.h"
#include "models/model_sync.h"
#include "models/model_resolver.h"
#include "models/model_registry.h"
#include "models/model_storage.h"
#include "core/llama_manager.h"
#include "core/inference_engine.h"
#include "api/openai_endpoints.h"
#include "api/node_endpoints.h"
#include "api/http_server.h"
#include "utils/config.h"
#include "utils/cli.h"
#include "utils/version.h"
#include "runtime/state.h"
#include "utils/logger.h"
#include "cli/commands.h"
#include "cli/ollama_compat.h"

#include <cstdlib>

namespace {

struct HfModelRef {
    std::string repo;
    std::string filename;
};

std::string toLowerAscii(std::string value) {
    std::transform(value.begin(), value.end(), value.begin(),
                   [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return value;
}

bool endsWith(const std::string& value, const std::string& suffix) {
    if (value.size() < suffix.size()) return false;
    return value.compare(value.size() - suffix.size(), suffix.size(), suffix) == 0;
}

std::optional<std::string> hostFromUrl(const std::string& url) {
    auto scheme_pos = url.find("://");
    if (scheme_pos == std::string::npos) return std::nullopt;
    std::string rest = url.substr(scheme_pos + 3);
    auto slash = rest.find('/');
    std::string host = (slash == std::string::npos) ? rest : rest.substr(0, slash);
    if (host.empty()) return std::nullopt;
    return host;
}

bool isHfHost(const std::string& host) {
    const std::string lower = toLowerAscii(host);
    if (lower == "huggingface.co" || endsWith(lower, ".huggingface.co")) {
        return true;
    }
    if (const char* base = std::getenv("HF_BASE_URL")) {
        std::string base_url(base);
        auto base_host = hostFromUrl(base_url);
        if (base_host && toLowerAscii(*base_host) == lower) {
            return true;
        }
    }
    return false;
}

std::vector<std::string> splitPath(const std::string& path) {
    std::vector<std::string> parts;
    size_t start = 0;
    while (start < path.size()) {
        size_t pos = path.find('/', start);
        std::string segment = (pos == std::string::npos)
            ? path.substr(start)
            : path.substr(start, pos - start);
        if (!segment.empty()) {
            parts.push_back(segment);
        }
        if (pos == std::string::npos) break;
        start = pos + 1;
    }
    return parts;
}

std::string joinPath(const std::vector<std::string>& parts, size_t start_index) {
    std::string out;
    for (size_t i = start_index; i < parts.size(); ++i) {
        if (!out.empty()) out.push_back('/');
        out.append(parts[i]);
    }
    return out;
}

std::optional<HfModelRef> parseHfModelRef(const std::string& input) {
    if (input.empty()) return std::nullopt;

    if (input.find("://") == std::string::npos) {
        std::string repo = input;
        auto qpos = repo.find_first_of("?#");
        if (qpos != std::string::npos) {
            repo = repo.substr(0, qpos);
        }
        if (!repo.empty() && repo.back() == '/') {
            repo.pop_back();
        }
        auto parts = splitPath(repo);
        if (parts.size() < 2) return std::nullopt;
        HfModelRef ref;
        ref.repo = parts[0] + "/" + parts[1];
        if (parts.size() > 2) {
            ref.filename = joinPath(parts, 2);
        }
        return ref;
    }

    auto host = hostFromUrl(input);
    if (!host || !isHfHost(*host)) return std::nullopt;

    auto scheme_pos = input.find("://");
    std::string rest = input.substr(scheme_pos + 3);
    auto slash = rest.find('/');
    if (slash == std::string::npos) return std::nullopt;
    std::string path = rest.substr(slash + 1);

    // Strip query/fragment
    auto qpos = path.find_first_of("?#");
    if (qpos != std::string::npos) {
        path = path.substr(0, qpos);
    }

    auto parts = splitPath(path);
    if (parts.size() < 2) return std::nullopt;

    HfModelRef ref;
    ref.repo = parts[0] + "/" + parts[1];

    if (parts.size() > 2) {
        const std::string& marker = parts[2];
        if ((marker == "resolve" || marker == "blob" || marker == "raw") && parts.size() >= 5) {
            ref.filename = joinPath(parts, 4);
        } else if (marker != "tree") {
            ref.filename = joinPath(parts, 2);
        }
    }

    return ref;
}

}  // namespace

#ifdef USE_WHISPER
#include "core/audio_manager.h"
#include "api/audio_endpoints.h"
#endif

#ifdef USE_ONNX_RUNTIME
#include "core/onnx_tts_manager.h"
#endif

#ifdef USE_SD
#include "core/image_manager.h"
#include "api/image_endpoints.h"
#endif

int run_node(const xllm::NodeConfig& cfg, bool single_iteration) {
    xllm::g_running_flag.store(true);

    bool server_started = false;
    bool llama_backend_initialized = false;

    try {
        xllm::logger::init_from_env();
        xllm::set_ready(false);
        int node_port = cfg.node_port;

        // Initialize llama.cpp backend
        spdlog::info("Initializing llama.cpp backend...");
        xllm::LlamaManager::initBackend();
        llama_backend_initialized = true;

        spdlog::info("Node port: {}", node_port);

        // GPU detection
        std::cout << "Detecting GPUs..." << std::endl;
        xllm::GpuDetector gpu_detector;
        auto gpus = gpu_detector.detect();
        if (cfg.require_gpu && !gpu_detector.hasGpu()) {
            std::cerr << "Error: No GPU detected. GPU is required for node operation." << std::endl;
            return 1;
        }
        size_t total_mem = gpu_detector.getTotalMemory();
        double capability = gpu_detector.getCapabilityScore();
        std::cout << "GPU detected: devices=" << gpus.size() << " total_mem=" << total_mem << " bytes" << std::endl;

        std::string bind_address = cfg.bind_address.empty() ? std::string("0.0.0.0") : cfg.bind_address;

        // Initialize model registry (empty for now, will sync after registration)
        xllm::ModelRegistry registry;
        registry.setGpuBackend(gpu_detector.getGpuBackend());

        // Determine models directory
        std::string models_dir = cfg.models_dir.empty()
                                     ? std::string(getenv("HOME") ? getenv("HOME") : ".") + "/.llm-router/models"
                                     : cfg.models_dir;

        // Initialize LlamaManager and ModelStorage for inference engine
        xllm::LlamaManager llama_manager(models_dir);
        xllm::ModelStorage model_storage(models_dir);

        std::vector<std::string> supported_runtimes{"llama_cpp"};

#ifdef USE_WHISPER
        // Initialize AudioManager for ASR
        xllm::AudioManager audio_manager(models_dir);
        spdlog::info("AudioManager initialized for ASR support");
        supported_runtimes.push_back("whisper_cpp");
#endif

#ifdef USE_ONNX_RUNTIME
        // Initialize OnnxTtsManager for TTS
        xllm::OnnxTtsManager tts_manager(models_dir);
        spdlog::info("OnnxTtsManager initialized for TTS support");
        supported_runtimes.push_back("onnx_runtime");
#endif

#ifdef USE_SD
        // Initialize ImageManager for image generation
        xllm::ImageManager image_manager(models_dir);
        spdlog::info("ImageManager initialized for image generation support");
        supported_runtimes.push_back("stable_diffusion");
#endif

        // Set GPU layers based on detection (use all layers on GPU if available)
        if (!gpus.empty()) {
            // Use 99 layers for GPU offloading (most models have fewer layers)
            llama_manager.setGpuLayerSplit(99);
            spdlog::info("GPU offloading enabled with {} layers", 99);
        }

        fprintf(stderr, "[DEBUG] main: starting on-demand config...\n");
        fflush(stderr);

        // Configure on-demand model loading settings from environment variables
        if (const char* idle_timeout_env = std::getenv("LLM_MODEL_IDLE_TIMEOUT")) {
            int timeout_secs = std::atoi(idle_timeout_env);
            if (timeout_secs > 0) {
                llama_manager.setIdleTimeout(std::chrono::seconds(timeout_secs));
                spdlog::info("Model idle timeout set to {} seconds", timeout_secs);
            }
        }
        if (const char* max_models_env = std::getenv("LLM_MAX_LOADED_MODELS")) {
            int max_models = std::atoi(max_models_env);
            if (max_models > 0) {
                llama_manager.setMaxLoadedModels(static_cast<size_t>(max_models));
                spdlog::info("Max loaded models set to {}", max_models);
            }
        }
        if (const char* max_memory_env = std::getenv("LLM_MAX_MEMORY_BYTES")) {
            long long max_memory = std::atoll(max_memory_env);
            if (max_memory > 0) {
                llama_manager.setMaxMemoryBytes(static_cast<size_t>(max_memory));
                spdlog::info("Max memory limit set to {} bytes", max_memory);
            }
        }

        fprintf(stderr, "[DEBUG] main: creating ResourceMonitor...\n");
        fflush(stderr);

        // Resource monitoring (VRAM/RAM watermark + LRU unload)
        xllm::ResourceMonitor resource_monitor([&llama_manager]() {
            if (xllm::active_request_count() > 0) {
                spdlog::info("Resource monitor: active requests in flight; skipping LRU unload");
                return false;
            }
            auto lru = llama_manager.getLeastRecentlyUsedModel();
            if (!lru.has_value()) {
                return false;
            }
            spdlog::warn("Resource monitor: unloading LRU model {}", lru.value());
            return llama_manager.unloadModel(lru.value());
        });
        resource_monitor.start();

        fprintf(stderr, "[DEBUG] main: ResourceMonitor started, creating ModelSync...\n");
        fflush(stderr);

        // Create model_sync for local model management (standalone mode - no router)
        auto model_sync = std::make_shared<xllm::ModelSync>("", models_dir);
        if (!cfg.origin_allowlist.empty()) {
            model_sync->setOriginAllowlist(cfg.origin_allowlist);
        }

        auto model_resolver = std::make_shared<xllm::ModelResolver>(
            cfg.models_dir,
            "",  // No router URL in standalone mode
            ""); // No API key needed
        if (!cfg.origin_allowlist.empty()) {
            model_resolver->setOriginAllowlist(cfg.origin_allowlist);
        }
        model_resolver->setSyncReporter(model_sync.get());

        fprintf(stderr, "[DEBUG] main: ModelResolver configured, creating InferenceEngine...\n");
        fflush(stderr);

        // Initialize inference engine with dependencies (ModelResolver handles local/manifest resolution)
        xllm::InferenceEngine engine(llama_manager, model_storage, model_sync.get(), model_resolver.get());

        fprintf(stderr, "[DEBUG] main: InferenceEngine created, registering runtimes...\n");
        fflush(stderr);

        for (const auto& rt : engine.getRegisteredRuntimes()) {
            if (std::find(supported_runtimes.begin(), supported_runtimes.end(), rt) == supported_runtimes.end()) {
                supported_runtimes.push_back(rt);
            }
        }
        model_sync->setSupportedRuntimes(supported_runtimes);

        spdlog::info("InferenceEngine initialized with text managers");

        // Scan local models BEFORE starting server (router checks /v1/models during registration)
        {
            auto local_descriptors = model_storage.listAvailableDescriptors();
            std::vector<std::string> initial_models;
            initial_models.reserve(local_descriptors.size());
            for (const auto& desc : local_descriptors) {
                if (!engine.isModelSupported(desc)) {
                    continue;
                }
                initial_models.push_back(desc.name);
            }
            registry.setModels(initial_models);
            spdlog::info("Model scan: found {} supported models out of {} total",
                         initial_models.size(), local_descriptors.size());
        }

        // Start HTTP server BEFORE registration (router checks /v1/models endpoint)
        xllm::OpenAIEndpoints openai(registry, engine, cfg, gpu_detector.getGpuBackend());
        xllm::NodeEndpoints node_endpoints;
        node_endpoints.setGpuInfo(gpus.size(), total_mem, capability);
        node_endpoints.setGpuDevices(gpus);
        xllm::HttpServer server(node_port, openai, node_endpoints, bind_address);

#ifdef USE_WHISPER
        // Register audio endpoints for ASR (and TTS if available)
#ifdef USE_ONNX_RUNTIME
        xllm::AudioEndpoints audio_endpoints(audio_manager, tts_manager);
        spdlog::info("Audio endpoints registered for ASR + TTS");
#else
        xllm::AudioEndpoints audio_endpoints(audio_manager);
        spdlog::info("Audio endpoints registered for ASR");
#endif
        audio_endpoints.registerRoutes(server.getServer());
#endif

#ifdef USE_SD
        // Register image endpoints for image generation
        xllm::ImageEndpoints image_endpoints(image_manager);
        image_endpoints.registerRoutes(server.getServer());
        spdlog::info("Image endpoints registered for image generation");
#endif

        // SPEC-dcaeaec4 FR-7: POST /api/models/pull - receive sync notification from router
        server.getServer().Post("/api/models/pull", [&model_sync, &model_storage, &registry, &engine](const httplib::Request& req, httplib::Response& res) {
            try {
                std::string requested;
                std::string filename_hint;
                auto body = nlohmann::json::parse(req.body, nullptr, false);
                if (!body.is_discarded()) {
                    if (body.contains("name") && body["name"].is_string()) {
                        requested = body["name"].get<std::string>();
                    } else if (body.contains("model") && body["model"].is_string()) {
                        requested = body["model"].get<std::string>();
                    }
                }

                if (!requested.empty()) {
                    auto ref = parseHfModelRef(requested);
                    if (!ref) {
                        res.status = 400;
                        nlohmann::json err;
                        err["error"] = "invalid HuggingFace model reference (expected owner/model or HuggingFace URL)";
                        res.set_content(err.dump(), "application/json");
                        return;
                    }

                    const std::string model_id = ref->repo;
                    filename_hint = ref->filename;
                    spdlog::info("Received model pull request: {}{}", model_id,
                                 filename_hint.empty() ? "" : " (" + filename_hint + ")");

                    xllm::ModelDownloader downloader("", model_sync->getModelsDir());
                    bool ok = model_sync->downloadModel(downloader, model_id, {}, filename_hint);
                    if (!ok) {
                        res.status = 500;
                        nlohmann::json err;
                        const auto msg = downloader.getLastError();
                        err["error"] = msg.empty() ? "download failed" : msg;
                        res.set_content(err.dump(), "application/json");
                        return;
                    }

                    // Update registry with current local models
                    auto local_descriptors = model_storage.listAvailableDescriptors();
                    std::vector<std::string> local_model_names;
                    local_model_names.reserve(local_descriptors.size());
                    for (const auto& desc : local_descriptors) {
                        if (!engine.isModelSupported(desc)) {
                            continue;
                        }
                        local_model_names.push_back(desc.name);
                    }
                    registry.setModels(local_model_names);
                    spdlog::info("Model pull completed, {} models available", local_model_names.size());

                    res.set_content(R"({"status":"ok"})", "application/json");
                    return;
                }

                spdlog::info("Received model pull notification from router");

                // Sync with router
                auto sync_result = model_sync->sync();

                // Skip model deletion - router catalog may not include all local models
                if (!sync_result.to_delete.empty()) {
                    spdlog::info("Skipping deletion of {} models not in router (local models preserved)",
                                 sync_result.to_delete.size());
                }

                // Update registry with current local models
                auto local_descriptors = model_storage.listAvailableDescriptors();
                std::vector<std::string> local_model_names;
                local_model_names.reserve(local_descriptors.size());
                for (const auto& desc : local_descriptors) {
                    if (!engine.isModelSupported(desc)) {
                        continue;
                    }
                    local_model_names.push_back(desc.name);
                }
                registry.setModels(local_model_names);
                spdlog::info("Model sync completed, {} models available", local_model_names.size());

                res.set_content(R"({"status":"ok"})", "application/json");
            } catch (const std::exception& e) {
                spdlog::error("Model pull failed: {}", e.what());
                res.status = 500;
                res.set_content(R"({"error":"sync failed"})", "application/json");
            }
        });
        spdlog::info("Model pull endpoint registered: POST /api/models/pull");

        // Initialize OllamaCompat for reading ~/.ollama/models/
        xllm::cli::OllamaCompat ollama_compat;

        // Ollama-compatible API: GET /api/tags - list all available models
        server.getServer().Get("/api/tags", [&model_storage, &ollama_compat, &engine](const httplib::Request&, httplib::Response& res) {
            nlohmann::json models_array = nlohmann::json::array();

            // List llm-router models
            auto descriptors = model_storage.listAvailableDescriptors();
            for (const auto& desc : descriptors) {
                if (!engine.isModelSupported(desc)) {
                    continue;
                }
                nlohmann::json model_obj;
                model_obj["name"] = desc.name;
                model_obj["model"] = desc.name;
                model_obj["modified_at"] = "";  // Could add file modification time
                model_obj["size"] = 0;  // Could add file size
                model_obj["digest"] = "";
                model_obj["details"] = nlohmann::json::object();
                model_obj["details"]["format"] = desc.format;
                model_obj["details"]["family"] = "";
                model_obj["details"]["parameter_size"] = "";
                model_obj["details"]["quantization_level"] = "";
                models_array.push_back(model_obj);
            }

            // List ollama models (read-only)
            if (ollama_compat.isAvailable()) {
                auto ollama_models = ollama_compat.listModels();
                for (const auto& info : ollama_models) {
                    nlohmann::json model_obj;
                    model_obj["name"] = "ollama:" + info.name;
                    model_obj["model"] = "ollama:" + info.name;
                    model_obj["modified_at"] = "(readonly)";
                    model_obj["size"] = static_cast<int64_t>(info.size_bytes);
                    model_obj["digest"] = info.blob_digest;
                    model_obj["details"] = nlohmann::json::object();
                    model_obj["details"]["format"] = "gguf";
                    model_obj["details"]["family"] = "";
                    model_obj["details"]["parameter_size"] = "";
                    model_obj["details"]["quantization_level"] = "";
                    models_array.push_back(model_obj);
                }
            }

            nlohmann::json response;
            response["models"] = models_array;
            res.set_content(response.dump(), "application/json");
        });
        spdlog::info("Ollama-compatible endpoint registered: GET /api/tags");

        // Ollama-compatible API: GET /api/ps - list running models
        server.getServer().Get("/api/ps", [&llama_manager](const httplib::Request&, httplib::Response& res) {
            nlohmann::json models_array = nlohmann::json::array();

            auto loaded_models = llama_manager.getLoadedModels();
            for (const auto& model_path : loaded_models) {
                // Extract model name from path
                std::filesystem::path p(model_path);
                std::string model_name = p.parent_path().filename().string();
                if (model_name.empty()) {
                    model_name = p.stem().string();
                }

                nlohmann::json model_obj;
                model_obj["name"] = model_name;
                model_obj["model"] = model_name;
                model_obj["size"] = 0;  // Could calculate actual size
                model_obj["digest"] = "";
                model_obj["details"] = nlohmann::json::object();
                model_obj["expires_at"] = "";  // Could add expiry based on idle timeout
                model_obj["size_vram"] = static_cast<int64_t>(llama_manager.memoryUsageBytes());
                models_array.push_back(model_obj);
            }

            nlohmann::json response;
            response["models"] = models_array;
            res.set_content(response.dump(), "application/json");
        });
        spdlog::info("Ollama-compatible endpoint registered: GET /api/ps");

        // Ollama-compatible API: POST /api/show - show model information
        server.getServer().Post("/api/show", [&model_storage, &ollama_compat](const httplib::Request& req, httplib::Response& res) {
            auto body = nlohmann::json::parse(req.body, nullptr, false);
            if (body.is_discarded() || !body.contains("name")) {
                res.status = 400;
                res.set_content(R"({"error":"name required"})", "application/json");
                return;
            }

            std::string model_name = body["name"].get<std::string>();
            nlohmann::json response;

            // Check if it's an ollama model
            if (xllm::cli::OllamaCompat::hasOllamaPrefix(model_name)) {
                std::string ollama_name = xllm::cli::OllamaCompat::stripOllamaPrefix(model_name);
                auto info = ollama_compat.getModel(ollama_name);
                if (info) {
                    response["modelfile"] = "";
                    response["parameters"] = "";
                    response["template"] = "";
                    response["details"] = nlohmann::json::object();
                    response["details"]["format"] = "gguf";
                    response["details"]["family"] = "";
                    response["details"]["parameter_size"] = "";
                    response["details"]["quantization_level"] = "";
                    response["model_info"] = nlohmann::json::object();
                    response["model_info"]["source"] = "ollama (read-only)";
                    response["model_info"]["blob_path"] = info->blob_path;
                    response["model_info"]["size_bytes"] = static_cast<int64_t>(info->size_bytes);
                    res.set_content(response.dump(), "application/json");
                    return;
                }
            }

            // Check llm-router models
            auto descriptor = model_storage.resolveDescriptor(model_name);
            if (descriptor) {
                response["modelfile"] = "";
                response["parameters"] = "";
                response["template"] = "";
                response["details"] = nlohmann::json::object();
                response["details"]["format"] = descriptor->format;
                response["details"]["family"] = "";
                response["details"]["parameter_size"] = "";
                response["details"]["quantization_level"] = "";
                response["model_info"] = nlohmann::json::object();
                response["model_info"]["name"] = descriptor->name;
                response["model_info"]["path"] = descriptor->primary_path;
                response["model_info"]["runtime"] = descriptor->runtime;
                res.set_content(response.dump(), "application/json");
                return;
            }

            res.status = 404;
            res.set_content(R"({"error":"model not found"})", "application/json");
        });
        spdlog::info("Ollama-compatible endpoint registered: POST /api/show");

        std::cout << "Starting HTTP server on port " << node_port << "..." << std::endl;
        server.start();
        server_started = true;

        // Wait for server to be ready by self-connecting
        {
            httplib::Client self_check("127.0.0.1", node_port);
            self_check.set_connection_timeout(1, 0);
            self_check.set_read_timeout(1, 0);
            const int max_wait = 50;  // 50 * 100ms = 5s max
            for (int i = 0; i < max_wait; ++i) {
                auto res = self_check.Get("/v1/models");
                if (res && res->status == 200) {
                    spdlog::info("Server ready after {}ms", (i + 1) * 100);
                    break;
                }
                std::this_thread::sleep_for(std::chrono::milliseconds(100));
            }
        }

        // xLLM now operates in standalone mode only (NodeRegistry abolished)
        // Models are scanned from local storage without router registration
        spdlog::info("Running in standalone mode (xLLM OpenAI-compatible endpoint)");

        // Update registry with local models (models actually available on this node)
        auto local_descriptors = model_storage.listAvailableDescriptors();
        std::vector<std::string> local_model_names;
        local_model_names.reserve(local_descriptors.size());
        for (const auto& desc : local_descriptors) {
            if (!engine.isModelSupported(desc)) {
                continue;
            }
            local_model_names.push_back(desc.name);
        }
        registry.setModels(local_model_names);
        spdlog::info("Registered {} local models", local_model_names.size());

        xllm::set_ready(true);

        std::cout << "Node initialized successfully, ready to serve requests" << std::endl;

        // Main loop
        if (single_iteration) {
            std::this_thread::sleep_for(std::chrono::milliseconds(500));
            xllm::request_shutdown();
        }
        while (xllm::is_running()) {
            std::this_thread::sleep_for(std::chrono::seconds(1));
        }

        // Cleanup
        std::cout << "Shutting down..." << std::endl;
        server.stop();
        resource_monitor.stop();

        // Free llama.cpp backend
        if (llama_backend_initialized) {
            spdlog::info("Freeing llama.cpp backend...");
            xllm::LlamaManager::freeBackend();
        }

    } catch (const std::exception& e) {
        std::cerr << "Fatal error: " << e.what() << std::endl;
        if (llama_backend_initialized) {
            xllm::LlamaManager::freeBackend();
        }
        if (server_started) {
            // best-effort stop
        }
        return 1;
    }

    std::cout << "Node shutdown complete" << std::endl;
    return 0;
}

void signalHandler(int signal) {
    std::cout << "Received signal " << signal << ", shutting down..." << std::endl;
    xllm::request_shutdown();
}

#ifndef XLLM_TESTING
int main(int argc, char* argv[]) {
    // Parse CLI arguments first
    auto cli_result = xllm::parseCliArgs(argc, argv);
    if (cli_result.should_exit) {
        std::cout << cli_result.output;
        return cli_result.exit_code;
    }

    // Set up signal handlers
    signal(SIGINT, signalHandler);
    signal(SIGTERM, signalHandler);

    // Branch based on subcommand
    switch (cli_result.subcommand) {
        case xllm::Subcommand::Serve: {
            std::cout << "xllm v" << XLLM_VERSION << " starting..." << std::endl;
            auto cfg = xllm::loadNodeConfig();
            // Override config with CLI options if specified
            if (cli_result.serve_options.port != 0) {
                cfg.node_port = cli_result.serve_options.port;
            }
            if (!cli_result.serve_options.host.empty()) {
                cfg.bind_address = cli_result.serve_options.host;
            }
            return run_node(cfg, /*single_iteration=*/false);
        }

        case xllm::Subcommand::Run:
            return xllm::cli::commands::run(cli_result.run_options);

        case xllm::Subcommand::Pull:
            return xllm::cli::commands::pull(cli_result.pull_options);

        case xllm::Subcommand::List:
            return xllm::cli::commands::list(cli_result.model_options);

        case xllm::Subcommand::Show:
            return xllm::cli::commands::show(cli_result.show_options);

        case xllm::Subcommand::Rm:
            return xllm::cli::commands::rm(cli_result.model_options);

        case xllm::Subcommand::Stop:
            return xllm::cli::commands::stop(cli_result.model_options);

        case xllm::Subcommand::Ps:
            return xllm::cli::commands::ps();

        case xllm::Subcommand::RouterEndpoints:
            return xllm::cli::commands::router_endpoints();

        case xllm::Subcommand::RouterModels:
            return xllm::cli::commands::router_models();

        case xllm::Subcommand::RouterStatus:
            return xllm::cli::commands::router_status();

        case xllm::Subcommand::None:
        default:
            // Default to serve (legacy behavior for backward compatibility)
            std::cout << "xllm v" << XLLM_VERSION << " starting..." << std::endl;
            auto cfg = xllm::loadNodeConfig();
            return run_node(cfg, /*single_iteration=*/false);
    }
}
#endif

#ifdef XLLM_TESTING
extern "C" int xllm_run_for_test() {
    auto cfg = xllm::loadNodeConfig();
    cfg.require_gpu = false;
    return run_node(cfg, /*single_iteration=*/true);
}
#endif
