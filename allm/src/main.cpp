#include <iostream>
#include <memory>
#include <signal.h>
#include <atomic>
#include <thread>
#include <chrono>
#include <string>
#include <vector>
#include <filesystem>

#include "system/gpu_detector.h"
#include "system/resource_monitor.h"
#include "api/router_client.h"
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

#ifdef USE_WHISPER
#include "core/whisper_manager.h"
#include "api/audio_endpoints.h"
#endif

#ifdef USE_ONNX_RUNTIME
#include "core/onnx_tts_manager.h"
#endif

#ifdef USE_SD
#include "core/sd_manager.h"
#include "api/image_endpoints.h"
#endif

int run_node(const llm_node::NodeConfig& cfg, bool single_iteration) {
    llm_node::g_running_flag.store(true);

    bool server_started = false;
    bool llama_backend_initialized = false;
    std::thread heartbeat_thread;

    try {
        llm_node::logger::init_from_env();
        llm_node::set_ready(false);
        std::string router_url = cfg.router_url;
        int node_port = cfg.node_port;

        // Initialize llama.cpp backend
        spdlog::info("Initializing llama.cpp backend...");
        llm_node::LlamaManager::initBackend();
        llama_backend_initialized = true;

        spdlog::info("Router URL: {}", router_url);
        if (cfg.router_api_key.empty()) {
            spdlog::warn("Router API key not set; node registration will fail if router requires API key");
        } else {
            spdlog::info("Router API key configured");
        }
        spdlog::info("Node port: {}", node_port);

        // GPU detection
        std::cout << "Detecting GPUs..." << std::endl;
        llm_node::GpuDetector gpu_detector;
        auto gpus = gpu_detector.detect();
        if (cfg.require_gpu && !gpu_detector.hasGpu()) {
            std::cerr << "Error: No GPU detected. GPU is required for node operation." << std::endl;
            return 1;
        }
        size_t total_mem = gpu_detector.getTotalMemory();
        double capability = gpu_detector.getCapabilityScore();
        std::cout << "GPU detected: devices=" << gpus.size() << " total_mem=" << total_mem << " bytes" << std::endl;

        // Build GPU device info for router
        std::vector<llm_node::GpuDeviceInfoForRouter> gpu_devices;
        for (const auto& gpu : gpus) {
            if (gpu.is_available) {
                llm_node::GpuDeviceInfoForRouter device;
                device.model = gpu.name;
                device.count = 1;
                device.memory = gpu.memory_bytes;
                gpu_devices.push_back(device);
            }
        }

        // Get machine name from hostname
        char hostname_buf[256] = "localhost";
        gethostname(hostname_buf, sizeof(hostname_buf));

        std::string bind_address = cfg.bind_address.empty() ? std::string("0.0.0.0") : cfg.bind_address;

        // Initialize model registry (empty for now, will sync after registration)
        llm_node::ModelRegistry registry;
        registry.setGpuBackend(gpu_detector.getGpuBackend());

        // Determine models directory
        std::string models_dir = cfg.models_dir.empty()
                                     ? std::string(getenv("HOME") ? getenv("HOME") : ".") + "/.llm-router/models"
                                     : cfg.models_dir;

        // Initialize LlamaManager and ModelStorage for inference engine
        llm_node::LlamaManager llama_manager(models_dir);
        llm_node::ModelStorage model_storage(models_dir);

        std::vector<std::string> supported_runtimes{"llama_cpp"};

#ifdef USE_WHISPER
        // Initialize WhisperManager for ASR
        llm_node::WhisperManager whisper_manager(models_dir);
        spdlog::info("WhisperManager initialized for ASR support");
        supported_runtimes.push_back("whisper_cpp");
#endif

#ifdef USE_ONNX_RUNTIME
        // Initialize OnnxTtsManager for TTS
        llm_node::OnnxTtsManager tts_manager(models_dir);
        spdlog::info("OnnxTtsManager initialized for TTS support");
        supported_runtimes.push_back("onnx_runtime");
#endif

#ifdef USE_SD
        // Initialize SDManager for image generation
        llm_node::SDManager sd_manager(models_dir);
        spdlog::info("SDManager initialized for image generation support");
        supported_runtimes.push_back("stable_diffusion");
#endif

        // Set GPU layers based on detection (use all layers on GPU if available)
        if (!gpu_devices.empty()) {
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
        llm_node::ResourceMonitor resource_monitor([&llama_manager]() {
            if (llm_node::active_request_count() > 0) {
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

        // Create model_sync early for remote path resolution & initial sync
        auto model_sync = std::make_shared<llm_node::ModelSync>(router_url, models_dir);
        if (!cfg.router_api_key.empty()) {
            model_sync->setApiKey(cfg.router_api_key);
        }
        model_sync->setSupportedRuntimes(supported_runtimes);
        if (!cfg.origin_allowlist.empty()) {
            model_sync->setOriginAllowlist(cfg.origin_allowlist);
        }

        auto model_resolver = std::make_shared<llm_node::ModelResolver>(
            cfg.models_dir,
            router_url,
            cfg.router_api_key);
        if (!cfg.origin_allowlist.empty()) {
            model_resolver->setOriginAllowlist(cfg.origin_allowlist);
        }
        model_resolver->setSyncReporter(model_sync.get());

        fprintf(stderr, "[DEBUG] main: ModelResolver configured, creating InferenceEngine...\n");
        fflush(stderr);

        // Initialize inference engine with dependencies (ModelResolver handles local/manifest resolution)
        llm_node::InferenceEngine engine(llama_manager, model_storage, model_sync.get(), model_resolver.get());

        fprintf(stderr, "[DEBUG] main: InferenceEngine created, checking plugins...\n");
        fflush(stderr);

        if (!cfg.engine_plugins_dir.empty() && std::filesystem::exists(cfg.engine_plugins_dir)) {
            fprintf(stderr, "[DEBUG] main: loading plugins from %s...\n", cfg.engine_plugins_dir.c_str());
            fflush(stderr);
            std::string plugin_error;
            if (!engine.loadEnginePlugins(cfg.engine_plugins_dir, plugin_error)) {
                spdlog::warn("Engine plugins load failed: {}", plugin_error);
            } else {
                spdlog::info("Engine plugins loaded from {}", cfg.engine_plugins_dir);
                // Add plugin runtimes to supported_runtimes
                for (const auto& rt : engine.getRegisteredRuntimes()) {
                    if (std::find(supported_runtimes.begin(), supported_runtimes.end(), rt) == supported_runtimes.end()) {
                        supported_runtimes.push_back(rt);
                        spdlog::info("Added plugin runtime: {}", rt);
                    }
                }
                // Update model_sync with expanded supported_runtimes
                model_sync->setSupportedRuntimes(supported_runtimes);
            }
        }
        engine.setPluginRestartPolicy(
            std::chrono::seconds(cfg.plugin_restart_interval_sec),
            cfg.plugin_restart_request_limit);
        if (cfg.plugin_restart_interval_sec > 0 || cfg.plugin_restart_request_limit > 0) {
            spdlog::info(
                "Engine plugin restart policy: interval={}s requests={}",
                cfg.plugin_restart_interval_sec,
                cfg.plugin_restart_request_limit);
        }
        spdlog::info("InferenceEngine initialized with llama.cpp support");

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
        llm_node::OpenAIEndpoints openai(registry, engine, cfg, gpu_detector.getGpuBackend());
        llm_node::NodeEndpoints node_endpoints;
        node_endpoints.setGpuInfo(gpus.size(), total_mem, capability);
        llm_node::HttpServer server(node_port, openai, node_endpoints, bind_address);

#ifdef USE_WHISPER
        // Register audio endpoints for ASR (and TTS if available)
#ifdef USE_ONNX_RUNTIME
        llm_node::AudioEndpoints audio_endpoints(whisper_manager, tts_manager);
        spdlog::info("Audio endpoints registered for ASR + TTS");
#else
        llm_node::AudioEndpoints audio_endpoints(whisper_manager);
        spdlog::info("Audio endpoints registered for ASR");
#endif
        audio_endpoints.registerRoutes(server.getServer());
#endif

#ifdef USE_SD
        // Register image endpoints for image generation
        llm_node::ImageEndpoints image_endpoints(sd_manager);
        image_endpoints.registerRoutes(server.getServer());
        spdlog::info("Image endpoints registered for image generation");
#endif

        // SPEC-dcaeaec4 FR-7: POST /api/models/pull - receive sync notification from router
        server.getServer().Post("/api/models/pull", [&model_sync, &model_storage, &registry, &engine](const httplib::Request&, httplib::Response& res) {
            try {
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
        llm_node::cli::OllamaCompat ollama_compat;

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
            if (llm_node::cli::OllamaCompat::hasOllamaPrefix(model_name)) {
                std::string ollama_name = llm_node::cli::OllamaCompat::stripOllamaPrefix(model_name);
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

        // Try to register with router (fallback to standalone if unavailable)
        std::cout << "Registering with router..." << std::endl;
        {
            llm_node::RouterClient router(router_url);
            if (!cfg.router_api_key.empty()) {
                router.setApiKey(cfg.router_api_key);
            }
            llm_node::NodeInfo info;
            info.machine_name = hostname_buf;
            // Use configured IP, or extract host from router URL, or fallback to hostname
            if (!cfg.ip_address.empty()) {
                info.ip_address = cfg.ip_address;
            } else {
                // Extract host from router_url (e.g., "http://192.168.1.100:8081" -> "192.168.1.100")
                std::string host = router_url;
                auto proto_end = host.find("://");
                if (proto_end != std::string::npos) {
                    host = host.substr(proto_end + 3);
                }
                auto port_pos = host.find(':');
                if (port_pos != std::string::npos) {
                    host = host.substr(0, port_pos);
                }
                auto path_pos = host.find('/');
                if (path_pos != std::string::npos) {
                    host = host.substr(0, path_pos);
                }
                // If router is on localhost, use 127.0.0.1; otherwise use router's host
                if (host == "localhost") {
                    info.ip_address = "127.0.0.1";
                } else {
                    info.ip_address = host;
                }
            }
            spdlog::info("Node IP address: {}", info.ip_address);
            info.runtime_version = "1.0.0";  // llm-node runtime version
            // Router calculates API port as runtime_port + 1, so report node_port - 1
            info.runtime_port = static_cast<uint16_t>(node_port > 0 ? node_port - 1 : 32768);
            info.gpu_available = !gpu_devices.empty();
            info.gpu_devices = gpu_devices;
            if (!gpu_devices.empty()) {
                info.gpu_count = static_cast<uint32_t>(gpu_devices.size());
                info.gpu_model = gpu_devices[0].model;
            }
            info.supported_runtimes = supported_runtimes;
            llm_node::NodeRegistrationResult reg;
            const int reg_max = 3;
            for (int attempt = 0; attempt < reg_max; ++attempt) {
                reg = router.registerNode(info);
                if (reg.success) break;
                std::this_thread::sleep_for(std::chrono::milliseconds(200 * (attempt + 1)));
            }
            if (!reg.success) {
                // Fallback to standalone mode when router is unavailable
                spdlog::warn("Router registration failed: {} - continuing in standalone mode", reg.error);
                std::cout << "Router unavailable, running in standalone mode..." << std::endl;
            } else {
                // Successfully registered with router - sync models and start heartbeat

                // Sync models from router (model_sync already created earlier for remote path resolution & initial sync)
                std::cout << "Syncing models from router..." << std::endl;
                model_sync->setNodeToken(reg.node_token);
                auto sync_result = model_sync->sync();
                if (sync_result.to_download.empty() && sync_result.to_delete.empty() && model_sync->listLocalModels().empty()) {
                    // If nothing synced and no local models, treat as recoverable error and retry once
                    std::this_thread::sleep_for(std::chrono::milliseconds(200));
                    sync_result = model_sync->sync();
                }

                // Skip model deletion for now - router catalog may not include all local models
                // In future, consider a config flag to control this behavior
                if (!sync_result.to_delete.empty()) {
                    spdlog::info("Skipping deletion of {} models not in router (local models preserved)",
                                 sync_result.to_delete.size());
                }

                // Heartbeat thread (only when connected to router)
                std::cout << "Starting heartbeat thread..." << std::endl;
                std::string node_token = reg.node_token;
                heartbeat_thread = std::thread([&router, &llama_manager, node_id = reg.node_id, node_token, &cfg,
                                                model_sync,
                                                supported_runtimes
#ifdef USE_WHISPER
                                                , &whisper_manager
#endif
#ifdef USE_ONNX_RUNTIME
                                                , &tts_manager
#endif
                                                , &resource_monitor
                                                ]() {
                    while (llm_node::is_running()) {
                        // 現在ロードされているモデルを取得
                        auto loaded_models = llama_manager.getLoadedModels();
                        // TODO: Phase 4でモデルタイプ識別を実装後、loaded_embedding_modelsを分離
                        std::vector<std::string> loaded_embedding_models;
                        std::vector<std::string> loaded_asr_models;
                        std::vector<std::string> loaded_tts_models;
#ifdef USE_WHISPER
                        loaded_asr_models = whisper_manager.getLoadedModels();
#endif
#ifdef USE_ONNX_RUNTIME
                        loaded_tts_models = tts_manager.getLoadedModels();
#endif
                        std::optional<llm_node::SyncStatusForRouter> sync_payload;
                        if (model_sync) {
                            const auto status = model_sync->getStatus();
                            auto state_to_string = [](llm_node::SyncState state) {
                                switch (state) {
                                    case llm_node::SyncState::Idle:
                                        return std::string("idle");
                                    case llm_node::SyncState::Running:
                                        return std::string("running");
                                    case llm_node::SyncState::Success:
                                        return std::string("success");
                                    case llm_node::SyncState::Failed:
                                        return std::string("failed");
                                }
                                return std::string("idle");
                            };
                            llm_node::SyncStatusForRouter payload;
                            payload.state = state_to_string(status.state);
                            if (status.current_download.has_value()) {
                                payload.progress = llm_node::SyncProgressForRouter{
                                    status.current_download->model_id,
                                    status.current_download->file,
                                    static_cast<uint64_t>(status.current_download->downloaded_bytes),
                                    static_cast<uint64_t>(status.current_download->total_bytes),
                                };
                            }
                            sync_payload = payload;
                        }
                        std::optional<llm_node::HeartbeatMetrics> metrics;
                        const auto usage = resource_monitor.latestUsage();
                        if (usage.mem_total_bytes > 0 || usage.vram_total_bytes > 0) {
                            llm_node::HeartbeatMetrics data;
                            data.mem_used_bytes = usage.mem_used_bytes;
                            data.mem_total_bytes = usage.mem_total_bytes;
                            if (usage.vram_total_bytes > 0) {
                                data.gpu_utilization = usage.vramUsageRatio() * 100.0;
                            }
                            metrics = data;
                        }
                        router.sendHeartbeat(node_id, node_token, std::nullopt, metrics,
                                             loaded_models, loaded_embedding_models,
                                             loaded_asr_models, loaded_tts_models,
                                             supported_runtimes, sync_payload);
                        std::this_thread::sleep_for(std::chrono::seconds(cfg.heartbeat_interval_sec));
                    }
                });
            }
        }

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

        llm_node::set_ready(true);

        std::cout << "Node initialized successfully, ready to serve requests" << std::endl;

        // Main loop
        if (single_iteration) {
            std::this_thread::sleep_for(std::chrono::milliseconds(500));
            llm_node::request_shutdown();
        }
        while (llm_node::is_running()) {
            std::this_thread::sleep_for(std::chrono::seconds(1));
        }

        // Cleanup
        std::cout << "Shutting down..." << std::endl;
        server.stop();
        resource_monitor.stop();
        if (heartbeat_thread.joinable()) {
            heartbeat_thread.join();
        }

        // Free llama.cpp backend
        if (llama_backend_initialized) {
            spdlog::info("Freeing llama.cpp backend...");
            llm_node::LlamaManager::freeBackend();
        }

    } catch (const std::exception& e) {
        std::cerr << "Fatal error: " << e.what() << std::endl;
        if (heartbeat_thread.joinable()) {
            llm_node::request_shutdown();
            heartbeat_thread.join();
        }
        if (llama_backend_initialized) {
            llm_node::LlamaManager::freeBackend();
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
    llm_node::request_shutdown();
}

#ifndef LLM_NODE_TESTING
int main(int argc, char* argv[]) {
    // Parse CLI arguments first
    auto cli_result = llm_node::parseCliArgs(argc, argv);
    if (cli_result.should_exit) {
        std::cout << cli_result.output;
        return cli_result.exit_code;
    }

    // Set up signal handlers
    signal(SIGINT, signalHandler);
    signal(SIGTERM, signalHandler);

    // Branch based on subcommand
    switch (cli_result.subcommand) {
        case llm_node::Subcommand::Serve: {
            std::cout << "allm v" << LLM_NODE_VERSION << " starting..." << std::endl;
            auto cfg = llm_node::loadNodeConfig();
            // Override config with CLI options if specified
            if (cli_result.serve_options.port != 0) {
                cfg.node_port = cli_result.serve_options.port;
            }
            if (!cli_result.serve_options.host.empty()) {
                cfg.bind_address = cli_result.serve_options.host;
            }
            return run_node(cfg, /*single_iteration=*/false);
        }

        case llm_node::Subcommand::Run:
            return llm_node::cli::commands::run(cli_result.run_options);

        case llm_node::Subcommand::Pull:
            return llm_node::cli::commands::pull(cli_result.pull_options);

        case llm_node::Subcommand::List:
            return llm_node::cli::commands::list(cli_result.model_options);

        case llm_node::Subcommand::Show:
            return llm_node::cli::commands::show(cli_result.show_options);

        case llm_node::Subcommand::Rm:
            return llm_node::cli::commands::rm(cli_result.model_options);

        case llm_node::Subcommand::Stop:
            return llm_node::cli::commands::stop(cli_result.model_options);

        case llm_node::Subcommand::Ps:
            return llm_node::cli::commands::ps();

        case llm_node::Subcommand::RouterEndpoints:
            return llm_node::cli::commands::router_endpoints();

        case llm_node::Subcommand::RouterModels:
            return llm_node::cli::commands::router_models();

        case llm_node::Subcommand::RouterStatus:
            return llm_node::cli::commands::router_status();

        case llm_node::Subcommand::None:
        default:
            // Default to serve (legacy behavior for backward compatibility)
            std::cout << "allm v" << LLM_NODE_VERSION << " starting..." << std::endl;
            auto cfg = llm_node::loadNodeConfig();
            return run_node(cfg, /*single_iteration=*/false);
    }
}
#endif

#ifdef LLM_NODE_TESTING
extern "C" int llm_node_run_for_test() {
    auto cfg = llm_node::loadNodeConfig();
    // short intervals for tests
    cfg.heartbeat_interval_sec = 1;
    cfg.require_gpu = false;
    return run_node(cfg, /*single_iteration=*/true);
}

// Backward compatibility for older test binaries that still reference the
// previous symbol name.
extern "C" int ollama_node_run_for_test() {
    return llm_node_run_for_test();
}
#endif
