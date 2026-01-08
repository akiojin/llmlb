#include "cli/cli_client.h"
#include <spdlog/spdlog.h>

namespace llm_node {
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
        const char* env_port = std::getenv("LLM_NODE_PORT");
        port_ = env_port ? static_cast<uint16_t>(std::stoi(env_port)) : 32769;
    } else {
        port_ = port;
    }
}

CliClient::~CliClient() = default;

bool CliClient::isServerRunning() const {
    // TODO: Implement health check
    return false;
}

CliResponse<nlohmann::json> CliClient::listModels() {
    // TODO: Implement GET /api/tags
    return {CliError::GeneralError, "Not implemented", std::nullopt};
}

CliResponse<nlohmann::json> CliClient::showModel(const std::string& model_name) {
    // TODO: Implement POST /api/show
    (void)model_name;
    return {CliError::GeneralError, "Not implemented", std::nullopt};
}

CliResponse<void> CliClient::deleteModel(const std::string& model_name) {
    // TODO: Implement DELETE /api/delete
    (void)model_name;
    CliResponse<void> response;
    response.error = CliError::GeneralError;
    response.error_message = "Not implemented";
    return response;
}

CliResponse<void> CliClient::stopModel(const std::string& model_name) {
    // TODO: Implement POST /api/stop
    (void)model_name;
    CliResponse<void> response;
    response.error = CliError::GeneralError;
    response.error_message = "Not implemented";
    return response;
}

CliResponse<nlohmann::json> CliClient::listRunningModels() {
    // TODO: Implement GET /api/ps
    return {CliError::GeneralError, "Not implemented", std::nullopt};
}

CliResponse<void> CliClient::pullModel(const std::string& model_name, ProgressCallback progress_cb) {
    // TODO: Implement POST /api/pull with progress streaming
    (void)model_name;
    (void)progress_cb;
    CliResponse<void> response;
    response.error = CliError::GeneralError;
    response.error_message = "Not implemented";
    return response;
}

CliResponse<std::string> CliClient::chat(
    const std::string& model_name,
    const nlohmann::json& messages,
    StreamCallback stream_cb
) {
    // TODO: Implement POST /api/chat with streaming
    (void)model_name;
    (void)messages;
    (void)stream_cb;
    return {CliError::GeneralError, "Not implemented", std::nullopt};
}

std::string CliClient::buildUrl(const std::string& path) const {
    return "http://" + host_ + ":" + std::to_string(port_) + path;
}

CliResponse<nlohmann::json> CliClient::httpGet(const std::string& path) {
    // TODO: Implement HTTP GET
    (void)path;
    return {CliError::GeneralError, "Not implemented", std::nullopt};
}

CliResponse<nlohmann::json> CliClient::httpPost(const std::string& path, const nlohmann::json& body) {
    // TODO: Implement HTTP POST
    (void)path;
    (void)body;
    return {CliError::GeneralError, "Not implemented", std::nullopt};
}

CliResponse<void> CliClient::httpDelete(const std::string& path) {
    // TODO: Implement HTTP DELETE
    (void)path;
    CliResponse<void> response;
    response.error = CliError::GeneralError;
    response.error_message = "Not implemented";
    return response;
}

CliResponse<std::string> CliClient::httpPostStream(
    const std::string& path,
    const nlohmann::json& body,
    StreamCallback stream_cb
) {
    // TODO: Implement streaming HTTP POST
    (void)path;
    (void)body;
    (void)stream_cb;
    return {CliError::GeneralError, "Not implemented", std::nullopt};
}

}  // namespace cli
}  // namespace llm_node
