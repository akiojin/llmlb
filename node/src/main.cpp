#include <iostream>
#include <memory>
#include <signal.h>
#include <atomic>
#include <thread>
#include <chrono>
#include <string>
#include <vector>
#include <unistd.h>

#include "system/gpu_detector.h"
#include "api/router_client.h"
#include "models/model_sync.h"
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

        // Determine models directory
        std::string models_dir = cfg.models_dir.empty()
                                     ? std::string(getenv("HOME") ? getenv("HOME") : ".") + "/.llm-router/models"
                                     : cfg.models_dir;

        // Initialize LlamaManager and ModelStorage for inference engine
        llm_node::LlamaManager llama_manager(models_dir);
        llm_node::ModelStorage model_storage(models_dir);

        std::vector<std::string> supported_runtimes;
        supported_runtimes.push_back("llama_cpp");

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

        // Create model_sync early for remote path resolution & initial sync
        auto model_sync = std::make_shared<llm_node::ModelSync>(router_url, models_dir);

        // Initialize inference engine with dependencies (pass model_sync for remote path resolution)
        llm_node::InferenceEngine engine(llama_manager, model_storage, model_sync.get());
        spdlog::info("InferenceEngine initialized with llama.cpp support");

        // Start HTTP server BEFORE registration (router checks /v1/models endpoint)
        llm_node::OpenAIEndpoints openai(registry, engine, cfg);
        llm_node::NodeEndpoints node_endpoints;
        node_endpoints.setGpuInfo(gpus.size(), total_mem, capability);
        llm_node::HttpServer server(node_port, openai, node_endpoints, bind_address);

#ifdef USE_WHISPER
        // Register audio endpoints for ASR (and TTS if available)
#ifdef USE_ONNX_RUNTIME
        llm_node::AudioEndpoints audio_endpoints(whisper_manager, tts_manager, cfg);
        spdlog::info("Audio endpoints registered for ASR + TTS");
#else
        llm_node::AudioEndpoints audio_endpoints(whisper_manager, cfg);
        spdlog::info("Audio endpoints registered for ASR");
#endif
        audio_endpoints.registerRoutes(server.getServer());
#endif

#ifdef USE_SD
        // Register image endpoints for image generation
        llm_node::ImageEndpoints image_endpoints(sd_manager, cfg);
        image_endpoints.registerRoutes(server.getServer());
        spdlog::info("Image endpoints registered for image generation");
#endif

        // SPEC-dcaeaec4 FR-7: POST /api/models/pull - receive sync notification from router
        server.getServer().Post("/api/models/pull", [&model_sync, &model_storage, &registry](const httplib::Request&, httplib::Response& res) {
            try {
                spdlog::info("Received model pull notification from router");

                // Sync with router
                auto sync_result = model_sync->sync();

                // Delete models not in router
                for (const auto& model_id : sync_result.to_delete) {
                    spdlog::info("Deleting model not in router: {}", model_id);
                    model_storage.deleteModel(model_id);
                }

                // Update registry with current local models
                auto local_model_infos = model_storage.listAvailable();
                std::vector<std::string> local_model_names;
                local_model_names.reserve(local_model_infos.size());
                for (const auto& info : local_model_infos) {
                    local_model_names.push_back(info.name);
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

        // Register with router (retry)
        std::cout << "Registering with router..." << std::endl;
        llm_node::RouterClient router(router_url);
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
        info.runtime_port = static_cast<uint16_t>(node_port > 0 ? node_port - 1 : 11434);
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
            std::cerr << "Router registration failed after retries: " << reg.error << std::endl;
            server.stop();
            return 1;
        }

        // Sync models from router (model_sync already created earlier for remote path resolution & initial sync)
        std::cout << "Syncing models from router..." << std::endl;
        model_sync->setNodeToken(reg.node_token);
        auto sync_result = model_sync->sync();
        if (sync_result.to_download.empty() && sync_result.to_delete.empty() && model_sync->listLocalModels().empty()) {
            // If nothing synced and no local models, treat as recoverable error and retry once
            std::this_thread::sleep_for(std::chrono::milliseconds(200));
            sync_result = model_sync->sync();
        }

        // Delete models not in router (router is source of truth)
        for (const auto& model_id : sync_result.to_delete) {
            std::cout << "Deleting model not in router: " << model_id << std::endl;
            if (model_storage.deleteModel(model_id)) {
                std::cout << "  Deleted: " << model_id << std::endl;
            } else {
                std::cerr << "  Failed to delete: " << model_id << std::endl;
            }
        }

        // Update registry with local models (models actually available on this node)
        auto local_model_infos = model_storage.listAvailable();
        std::vector<std::string> local_model_names;
        local_model_names.reserve(local_model_infos.size());
        for (const auto& info : local_model_infos) {
            local_model_names.push_back(info.name);
        }
        registry.setModels(local_model_names);
        spdlog::info("Registered {} local models", local_model_names.size());

        llm_node::set_ready(true);

        // Heartbeat thread
        std::cout << "Starting heartbeat thread..." << std::endl;
        std::string node_token = reg.node_token;
        heartbeat_thread = std::thread([&router, &llama_manager, node_id = reg.node_id, node_token, &cfg,
                                        supported_runtimes
#ifdef USE_WHISPER
                                        , &whisper_manager
#endif
#ifdef USE_ONNX_RUNTIME
                                        , &tts_manager
#endif
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
                router.sendHeartbeat(node_id, node_token, std::nullopt, std::nullopt,
                                     loaded_models, loaded_embedding_models,
                                     loaded_asr_models, loaded_tts_models,
                                     supported_runtimes);
                std::this_thread::sleep_for(std::chrono::seconds(cfg.heartbeat_interval_sec));
            }
        });

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

    std::cout << "llm-node v" << LLM_NODE_VERSION << " starting..." << std::endl;

    auto cfg = llm_node::loadNodeConfig();
    return run_node(cfg, /*single_iteration=*/false);
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
