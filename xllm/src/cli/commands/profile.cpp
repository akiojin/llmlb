#include "cli/commands.h"

#include <chrono>
#include <iomanip>
#include <iostream>

#define CPPHTTPLIB_OPENSSL_SUPPORT
#include <httplib.h>
#include <nlohmann/json.hpp>

#include "cli/cli_client.h"

namespace xllm::cli::commands {

namespace {
struct BenchmarkSample {
    double seconds{0.0};
    int completion_tokens{0};
    double tokens_per_second{0.0};
};

bool run_completion(const std::string& model,
                    const std::string& prompt,
                    int max_tokens,
                    BenchmarkSample& out,
                    std::string& error) {
    CliClient client;
    if (!client.isServerRunning()) {
        error = "Server is not running";
        return false;
    }

    nlohmann::json body;
    body["model"] = model;
    body["messages"] = nlohmann::json::array({{{"role", "user"}, {"content", prompt}}});
    body["max_tokens"] = max_tokens;

    httplib::Client http(client.getHost(), client.getPort());
    http.set_connection_timeout(5, 0);
    http.set_read_timeout(60, 0);

    auto start = std::chrono::steady_clock::now();
    auto res = http.Post("/v1/chat/completions", body.dump(), "application/json");
    auto end = std::chrono::steady_clock::now();

    if (!res) {
        error = "Failed to connect to server";
        return false;
    }
    if (res->status != 200) {
        error = res->body;
        return false;
    }

    int completion_tokens = 0;
    try {
        auto json = nlohmann::json::parse(res->body);
        if (json.contains("usage") && json["usage"].contains("completion_tokens")) {
            completion_tokens = json["usage"]["completion_tokens"].get<int>();
        } else if (json.contains("choices") && json["choices"].is_array()) {
            const auto text = json["choices"][0]["message"]["content"].get<std::string>();
            completion_tokens = static_cast<int>(text.size() / 4);
        }
    } catch (...) {
        completion_tokens = static_cast<int>(res->body.size() / 4);
    }

    out.seconds = std::chrono::duration_cast<std::chrono::duration<double>>(end - start).count();
    out.completion_tokens = completion_tokens;
    out.tokens_per_second = out.seconds > 0.0 ? completion_tokens / out.seconds : 0.0;
    return true;
}
}

int profile(const ProfileOptions& options) {
    BenchmarkSample sample;
    std::string error;
    if (!run_completion(options.model, options.prompt, options.max_tokens, sample, error)) {
        std::cerr << "Error: " << error << "\n";
        return error == "Server is not running" ? 2 : 1;
    }

    std::cout << "Model: " << options.model << "\n";
    std::cout << "Latency: " << std::fixed << std::setprecision(3) << sample.seconds << "s\n";
    std::cout << "Tokens: " << sample.completion_tokens << "\n";
    std::cout << "Tokens/sec: " << std::fixed << std::setprecision(2) << sample.tokens_per_second << "\n";
    return 0;
}

}  // namespace xllm::cli::commands
