#pragma once

#include <string>
#include <functional>
#include <optional>
#include <vector>
#include <nlohmann/json.hpp>

namespace allm {
namespace cli {

/// Callback for streaming responses
using StreamCallback = std::function<void(const std::string& chunk)>;

/// Callback for download progress
using ProgressCallback = std::function<void(uint64_t downloaded, uint64_t total, double speed_bps)>;

/// Error codes for CLI operations
enum class CliError {
    Success = 0,
    GeneralError = 1,
    ConnectionError = 2,
};

/// Result of CLI operations (generic template)
template<typename T>
struct CliResponse {
    CliError error{CliError::Success};
    std::string error_message;
    std::optional<T> data;

    bool ok() const { return error == CliError::Success; }
};

/// Specialization for void type (no data member)
template<>
struct CliResponse<void> {
    CliError error{CliError::Success};
    std::string error_message;

    bool ok() const { return error == CliError::Success; }
};

/// CLI client for communicating with aLLM server
class CliClient {
public:
    /// Constructor
    /// @param host Server host (default from LLM_ROUTER_HOST env)
    /// @param port Server port
    explicit CliClient(const std::string& host = "", uint16_t port = 0);

    ~CliClient();

    /// Check if server is running
    bool isServerRunning() const;

    /// List available models
    CliResponse<nlohmann::json> listModels();

    /// Get model details
    CliResponse<nlohmann::json> showModel(const std::string& model_name);

    /// Delete a model
    CliResponse<void> deleteModel(const std::string& model_name);

    /// Stop (unload) a model
    CliResponse<void> stopModel(const std::string& model_name);

    /// List running models (ps)
    CliResponse<nlohmann::json> listRunningModels();

    /// Pull/download a model
    CliResponse<void> pullModel(const std::string& model_name, ProgressCallback progress_cb = nullptr);

    /// Send chat message (for REPL)
    CliResponse<std::string> chat(
        const std::string& model_name,
        const nlohmann::json& messages,
        StreamCallback stream_cb = nullptr
    );

    /// Get server host
    const std::string& getHost() const { return host_; }

    /// Get server port
    uint16_t getPort() const { return port_; }

private:
    std::string host_;
    uint16_t port_;

    /// Build base URL
    std::string buildUrl(const std::string& path) const;

    /// Make HTTP GET request
    CliResponse<nlohmann::json> httpGet(const std::string& path);

    /// Make HTTP POST request
    CliResponse<nlohmann::json> httpPost(const std::string& path, const nlohmann::json& body);

    /// Make HTTP DELETE request
    CliResponse<void> httpDelete(const std::string& path);

    /// Make streaming HTTP POST request
    CliResponse<std::string> httpPostStream(
        const std::string& path,
        const nlohmann::json& body,
        StreamCallback stream_cb
    );
};

}  // namespace cli
}  // namespace allm
