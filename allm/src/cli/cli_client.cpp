// SPEC-58378000: CLI client implementation
// HTTP client for communicating with llm-node server

#include "cli/cli_client.h"

#define CPPHTTPLIB_OPENSSL_SUPPORT
#include <httplib.h>
#include <spdlog/spdlog.h>

namespace allm {
namespace cli {

CliClient::CliClient(const std::string& host, uint16_t port) {
    // Get host from environment or use default
    if (host.empty()) {
        const char* env_host = std::getenv("LLM_ROUTER_HOST");
        host_ = env_host ? env_host : "127.0.0.1";
    } else {
        host_ = host;
    }

    // Get port from environment or use default
    if (port == 0) {
        const char* env_port = std::getenv("ALLM_PORT");
        port_ = env_port ? static_cast<uint16_t>(std::stoi(env_port)) : 32769;
    } else {
        port_ = port;
    }
}

CliClient::~CliClient() = default;

bool CliClient::isServerRunning() const {
    httplib::Client client(host_, port_);
    client.set_connection_timeout(2, 0);  // 2 seconds
    client.set_read_timeout(2, 0);

    auto res = client.Get("/health");
    return res && res->status == 200;
}

CliResponse<nlohmann::json> CliClient::listModels() {
    return httpGet("/api/tags");
}

CliResponse<nlohmann::json> CliClient::showModel(const std::string& model_name) {
    nlohmann::json body;
    body["name"] = model_name;
    return httpPost("/api/show", body);
}

CliResponse<void> CliClient::deleteModel(const std::string& model_name) {
    httplib::Client client(host_, port_);
    client.set_connection_timeout(5, 0);
    client.set_read_timeout(30, 0);

    nlohmann::json body;
    body["name"] = model_name;

    auto res = client.Post("/api/delete", body.dump(), "application/json");

    CliResponse<void> response;
    if (!res) {
        response.error = CliError::ConnectionError;
        response.error_message = "Failed to connect to server";
        return response;
    }

    if (res->status != 200) {
        response.error = CliError::GeneralError;
        try {
            auto json = nlohmann::json::parse(res->body);
            response.error_message = json.value("error", res->body);
        } catch (...) {
            response.error_message = res->body;
        }
        return response;
    }

    response.error = CliError::Success;
    return response;
}

CliResponse<void> CliClient::stopModel(const std::string& model_name) {
    httplib::Client client(host_, port_);
    client.set_connection_timeout(5, 0);
    client.set_read_timeout(30, 0);

    nlohmann::json body;
    body["name"] = model_name;

    auto res = client.Post("/api/generate", body.dump(), "application/json");

    CliResponse<void> response;
    if (!res) {
        response.error = CliError::ConnectionError;
        response.error_message = "Failed to connect to server";
        return response;
    }

    if (res->status != 200) {
        response.error = CliError::GeneralError;
        try {
            auto json = nlohmann::json::parse(res->body);
            response.error_message = json.value("error", res->body);
        } catch (...) {
            response.error_message = res->body;
        }
        return response;
    }

    response.error = CliError::Success;
    return response;
}

CliResponse<nlohmann::json> CliClient::listRunningModels() {
    return httpGet("/api/ps");
}

CliResponse<void> CliClient::pullModel(const std::string& model_name, ProgressCallback progress_cb) {
    httplib::Client client(host_, port_);
    client.set_connection_timeout(10, 0);
    client.set_read_timeout(0, 0);  // No timeout for long downloads

    nlohmann::json body;
    body["name"] = model_name;
    body["stream"] = true;

    CliResponse<void> response;

    // Use Post with ContentReceiver for streaming progress
    auto res = client.Post(
        "/api/models/pull",
        httplib::Headers{},
        body.dump(),
        "application/json",
        [&progress_cb](const char* data, size_t len) -> bool {
            if (!progress_cb) return true;

            std::string chunk(data, len);

            // Parse NDJSON progress updates
            std::istringstream stream(chunk);
            std::string line;
            while (std::getline(stream, line)) {
                if (line.empty()) continue;
                try {
                    auto json = nlohmann::json::parse(line);
                    uint64_t completed = json.value("completed", static_cast<uint64_t>(0));
                    uint64_t total = json.value("total", static_cast<uint64_t>(0));

                    // Calculate speed (approximation)
                    double speed = 0.0;
                    if (json.contains("speed")) {
                        speed = json["speed"].get<double>();
                    }

                    progress_cb(completed, total, speed);
                } catch (...) {
                    // Ignore parse errors in progress stream
                }
            }
            return true;
        }
    );

    if (!res) {
        response.error = CliError::ConnectionError;
        response.error_message = "Failed to connect to server";
        return response;
    }

    if (res->status != 200) {
        response.error = CliError::GeneralError;
        try {
            auto json = nlohmann::json::parse(res->body);
            response.error_message = json.value("error", res->body);
        } catch (...) {
            response.error_message = res->body;
        }
        return response;
    }

    response.error = CliError::Success;
    return response;
}

CliResponse<std::string> CliClient::chat(
    const std::string& model_name,
    const nlohmann::json& messages,
    StreamCallback stream_cb
) {
    nlohmann::json body;
    body["model"] = model_name;
    body["messages"] = messages;
    body["stream"] = (stream_cb != nullptr);

    return httpPostStream("/v1/chat/completions", body, stream_cb);
}

std::string CliClient::buildUrl(const std::string& path) const {
    return "http://" + host_ + ":" + std::to_string(port_) + path;
}

CliResponse<nlohmann::json> CliClient::httpGet(const std::string& path) {
    httplib::Client client(host_, port_);
    client.set_connection_timeout(5, 0);
    client.set_read_timeout(30, 0);

    auto res = client.Get(path);

    if (!res) {
        return {CliError::ConnectionError, "Failed to connect to server", std::nullopt};
    }

    if (res->status != 200) {
        std::string error_msg;
        try {
            auto json = nlohmann::json::parse(res->body);
            error_msg = json.value("error", res->body);
        } catch (...) {
            error_msg = res->body;
        }
        return {CliError::GeneralError, error_msg, std::nullopt};
    }

    try {
        auto json = nlohmann::json::parse(res->body);
        return {CliError::Success, "", json};
    } catch (const std::exception& e) {
        return {CliError::GeneralError, std::string("JSON parse error: ") + e.what(), std::nullopt};
    }
}

CliResponse<nlohmann::json> CliClient::httpPost(const std::string& path, const nlohmann::json& body) {
    httplib::Client client(host_, port_);
    client.set_connection_timeout(5, 0);
    client.set_read_timeout(30, 0);

    auto res = client.Post(path, body.dump(), "application/json");

    if (!res) {
        return {CliError::ConnectionError, "Failed to connect to server", std::nullopt};
    }

    if (res->status != 200) {
        std::string error_msg;
        try {
            auto json = nlohmann::json::parse(res->body);
            error_msg = json.value("error", res->body);
        } catch (...) {
            error_msg = res->body;
        }
        return {CliError::GeneralError, error_msg, std::nullopt};
    }

    try {
        auto json = nlohmann::json::parse(res->body);
        return {CliError::Success, "", json};
    } catch (const std::exception& e) {
        return {CliError::GeneralError, std::string("JSON parse error: ") + e.what(), std::nullopt};
    }
}

CliResponse<void> CliClient::httpDelete(const std::string& path) {
    httplib::Client client(host_, port_);
    client.set_connection_timeout(5, 0);
    client.set_read_timeout(30, 0);

    auto res = client.Delete(path);

    CliResponse<void> response;
    if (!res) {
        response.error = CliError::ConnectionError;
        response.error_message = "Failed to connect to server";
        return response;
    }

    if (res->status != 200 && res->status != 204) {
        response.error = CliError::GeneralError;
        try {
            auto json = nlohmann::json::parse(res->body);
            response.error_message = json.value("error", res->body);
        } catch (...) {
            response.error_message = res->body;
        }
        return response;
    }

    response.error = CliError::Success;
    return response;
}

CliResponse<std::string> CliClient::httpPostStream(
    const std::string& path,
    const nlohmann::json& body,
    StreamCallback stream_cb
) {
    httplib::Client client(host_, port_);
    client.set_connection_timeout(5, 0);
    client.set_read_timeout(0, 0);  // No timeout for streaming

    std::string full_response;

    // Use Post with ContentReceiver for streaming
    auto res = client.Post(
        path,
        httplib::Headers{},
        body.dump(),
        "application/json",
        [&stream_cb, &full_response](const char* data, size_t len) -> bool {
            std::string chunk(data, len);

            // Parse SSE data
            std::istringstream stream(chunk);
            std::string line;
            while (std::getline(stream, line)) {
                if (line.empty() || line == "data: [DONE]") continue;

                // Remove "data: " prefix if present
                if (line.substr(0, 6) == "data: ") {
                    line = line.substr(6);
                }

                try {
                    auto json = nlohmann::json::parse(line);
                    if (json.contains("choices") && !json["choices"].empty()) {
                        auto& choice = json["choices"][0];
                        if (choice.contains("delta") && choice["delta"].contains("content")) {
                            std::string content = choice["delta"]["content"].get<std::string>();
                            full_response += content;
                            if (stream_cb) {
                                stream_cb(content);
                            }
                        }
                    }
                } catch (...) {
                    // Ignore parse errors in stream
                }
            }
            return true;
        }
    );

    if (!res) {
        return {CliError::ConnectionError, "Failed to connect to server", std::nullopt};
    }

    if (res->status != 200) {
        std::string error_msg;
        try {
            auto json = nlohmann::json::parse(res->body);
            error_msg = json.value("error", res->body);
        } catch (...) {
            error_msg = res->body;
        }
        return {CliError::GeneralError, error_msg, std::nullopt};
    }

    return {CliError::Success, "", full_response};
}

}  // namespace cli
}  // namespace allm
