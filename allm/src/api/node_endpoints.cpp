#include "api/node_endpoints.h"

#include <deque>
#include <fstream>
#include <stdexcept>
#include <nlohmann/json.hpp>
#include "runtime/state.h"
#include "system/resource_monitor.h"
#include "utils/logger.h"

namespace allm {

NodeEndpoints::NodeEndpoints() : health_status_("ok") {}

void NodeEndpoints::setGpuDevices(std::vector<GpuDevice> devices) {
    gpu_devices_ = std::move(devices);
}

void NodeEndpoints::registerRoutes(httplib::Server& server) {
    start_time_ = std::chrono::steady_clock::now();

    server.Get("/v0/logs", [](const httplib::Request& req, httplib::Response& res) {
        int limit = 200;
        if (req.has_param("tail")) {
            try {
                limit = std::stoi(req.get_param_value("tail"));
            } catch (...) {
                limit = 200;
            }
        }
        if (limit < 1) limit = 1;
        if (limit > 1000) limit = 1000;

        const std::string log_path = logger::get_log_file_path();

        nlohmann::json body;
        body["entries"] = nlohmann::json::array();
        body["path"] = log_path;

        std::ifstream file(log_path);
        if (!file.is_open()) {
            res.set_content(body.dump(), "application/json");
            return;
        }

        std::deque<nlohmann::json> entries;
        std::string line;
        while (std::getline(file, line)) {
            auto j = nlohmann::json::parse(line, nullptr, false);
            if (j.is_discarded() || !j.is_object()) continue;

            nlohmann::json entry;

            // timestamp: accept both tracing-subscriber style and node JSONL style
            if (j.contains("timestamp")) {
                entry["timestamp"] = j["timestamp"];
            } else if (j.contains("ts")) {
                entry["timestamp"] = j["ts"];
            } else {
                entry["timestamp"] = nullptr;
            }

            if (j.contains("level")) {
                entry["level"] = j["level"];
            } else {
                entry["level"] = nullptr;
            }

            if (j.contains("target")) {
                entry["target"] = j["target"];
            } else {
                entry["target"] = "allm";
            }

            // fields
            if (j.contains("fields") && j["fields"].is_object()) {
                entry["fields"] = j["fields"];
            } else {
                entry["fields"] = nlohmann::json::object();
            }

            // message: prefer explicit message, then msg, then fields.message
            if (j.contains("message")) {
                entry["message"] = j["message"];
            } else if (j.contains("msg")) {
                entry["message"] = j["msg"];
            } else if (entry["fields"].is_object() && entry["fields"].contains("message")) {
                entry["message"] = entry["fields"]["message"];
            } else {
                entry["message"] = nullptr;
            }

            if (j.contains("file")) {
                entry["file"] = j["file"];
            }
            if (j.contains("line")) {
                entry["line"] = j["line"];
            }

            entries.push_back(entry);
            if (static_cast<int>(entries.size()) > limit) {
                entries.pop_front();
            }
        }

        for (const auto& e : entries) {
            body["entries"].push_back(e);
        }
        res.set_content(body.dump(), "application/json");
    });

    server.Get("/health", [this](const httplib::Request&, httplib::Response& res) {
        nlohmann::json body = {{"status", health_status_}};
        res.set_content(body.dump(), "application/json");
    });

    // Phase 1.2: GET /v0/health - Extended health endpoint with GPU and load info
    server.Get("/v0/health", [this](const httplib::Request&, httplib::Response& res) {
        // Determine status based on readiness and active requests
        std::string status;
        unsigned int active_reqs = active_request_count();
        if (!is_ready()) {
            status = "offline";
        } else if (active_reqs >= 1) {
            status = "busy";
        } else {
            status = "online";
        }

        // Get current resource usage for VRAM info
        auto usage = ResourceMonitor::sampleSystemUsage();

        // Build GPU devices array
        nlohmann::json gpu_devices_json = nlohmann::json::array();
        for (const auto& dev : gpu_devices_) {
            gpu_devices_json.push_back({
                {"id", dev.id},
                {"name", dev.name},
                {"vendor", dev.vendor},
                {"memory_bytes", dev.memory_bytes},
                {"free_memory_bytes", dev.free_memory_bytes},
                {"is_available", dev.is_available}
            });
        }

        nlohmann::json body = {
            {"status", status},
            {"gpu", {
                {"device_count", gpu_devices_.size()},
                {"total_memory_bytes", gpu_total_mem_},
                {"used_memory_bytes", usage.vram_used_bytes},
                {"free_memory_bytes", usage.vram_total_bytes > usage.vram_used_bytes
                    ? usage.vram_total_bytes - usage.vram_used_bytes : 0},
                {"capability_score", gpu_capability_},
                {"devices", gpu_devices_json}
            }},
            {"load", {
                {"active_requests", active_reqs},
                {"max_concurrent_requests", 1}
            }},
            {"memory", {
                {"ram_used_bytes", usage.mem_used_bytes},
                {"ram_total_bytes", usage.mem_total_bytes},
                {"vram_used_bytes", usage.vram_used_bytes},
                {"vram_total_bytes", usage.vram_total_bytes}
            }}
        };
        res.set_content(body.dump(), "application/json");
    });

    server.Get("/startup", [](const httplib::Request&, httplib::Response& res) {
        if (allm::is_ready()) {
            res.set_content(R"({"status":"ready"})", "application/json");
        } else {
            res.status = 503;
            res.set_content(R"({"status":"starting"})", "application/json");
        }
    });

    server.Get("/metrics", [this](const httplib::Request&, httplib::Response& res) {
        auto uptime = std::chrono::duration_cast<std::chrono::seconds>(
            std::chrono::steady_clock::now() - start_time_).count();
        nlohmann::json body = {
            {"uptime_seconds", uptime},
            {"gpu_devices", gpu_devices_count_},
            {"gpu_memory_bytes", gpu_total_mem_},
            {"gpu_capability", gpu_capability_}
        };
        res.set_content(body.dump(), "application/json");
    });

    server.Get("/metrics/prom", [this](const httplib::Request&, httplib::Response& res) {
        auto uptime = std::chrono::duration_cast<std::chrono::seconds>(
            std::chrono::steady_clock::now() - start_time_).count();
        exporter_.set_gauge("allm_uptime_seconds", static_cast<double>(uptime), "Node uptime in seconds");
        exporter_.set_gauge("allm_gpu_devices", static_cast<double>(gpu_devices_count_), "Detected GPU devices");
        exporter_.set_gauge("allm_gpu_memory_bytes", static_cast<double>(gpu_total_mem_), "Total GPU memory bytes");
        exporter_.set_gauge("allm_gpu_capability", gpu_capability_, "Aggregated GPU capability score");
        res.set_content(exporter_.render(), "text/plain");
    });

    server.Get("/log/level", [](const httplib::Request&, httplib::Response& res) {
        nlohmann::json body = {{"level", spdlog::level::to_string_view(spdlog::get_level()).data()}};
        res.set_content(body.dump(), "application/json");
    });

    server.Post("/log/level", [](const httplib::Request& req, httplib::Response& res) {
        auto j = nlohmann::json::parse(req.body, nullptr, false);
        if (j.is_discarded() || !j.contains("level")) {
            res.status = 400;
            res.set_content(R"({"error":"level required"})", "application/json");
            return;
        }
        auto level_str = j["level"].get<std::string>();
        spdlog::set_level(logger::parse_level(level_str));
        nlohmann::json body = {{"level", spdlog::level::to_string_view(spdlog::get_level()).data()}};
        res.set_content(body.dump(), "application/json");
    });

    server.Get("/internal-error", [](const httplib::Request&, httplib::Response&) {
        throw std::runtime_error("boom");
    });
}

}  // namespace allm
