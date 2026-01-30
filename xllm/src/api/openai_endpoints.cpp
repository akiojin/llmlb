#include "api/openai_endpoints.h"

#include <cctype>
#include <nlohmann/json.hpp>
#include <limits>
#include <memory>
#include <cctype>
#include <algorithm>
#include <chrono>
#include <filesystem>
#include <random>
#include <sstream>
#include <iomanip>
#include "models/model_registry.h"
#include "models/modelfile.h"
#include "core/inference_engine.h"
#include "runtime/state.h"
#include "utils/utf8.h"

namespace xllm {

using json = nlohmann::json;

namespace {
// SPEC-dcaeaec4: Helper to check if node is ready and return 503 if not
bool checkReady(httplib::Response& res) {
    if (!is_ready()) {
        res.status = 503;
        nlohmann::json err = {
            {"error", {
                {"code", "service_unavailable"},
                {"type", "service_unavailable"},
                {"message", "Node is syncing models with router. Please wait."}
            }}
        };
        res.set_content(err.dump(), "application/json");
        return false;
    }
    return true;
}

std::chrono::milliseconds get_request_queue_timeout() {
    constexpr int64_t kDefaultTimeoutMs = 1000;
    if (const char* env = std::getenv("XLLM_REQUEST_QUEUE_TIMEOUT_MS")) {
        try {
            int64_t value = std::stoll(env);
            if (value > 0) {
                return std::chrono::milliseconds(value);
            }
        } catch (...) {
        }
    }
    return std::chrono::milliseconds(kDefaultTimeoutMs);
}

const char* get_json_grammar() {
    static const char* kJsonGrammar = R"GRAMMAR(
root   ::= object
value  ::= object | array | string | number | ("true" | "false" | "null") ws

object ::=
  "{" ws (
            string ":" ws value
    ("," ws string ":" ws value)*
  )? "}" ws

array  ::=
  "[" ws (
            value
    ("," ws value)*
  )? "]" ws

string ::=
  "\"" (
    [^"\\\x7F\x00-\x1F] |
    "\\" (["\\bfnrt] | "u" [0-9a-fA-F]{4})
  )* "\"" ws

number ::= ("-"? ([0-9] | [1-9] [0-9]{0,15})) ("." [0-9]+)? ([eE] [-+]? [0-9] [1-9]{0,15})? ws

ws ::= | " " | "\n" [ \t]{0,20}
)GRAMMAR";
    return kJsonGrammar;
}

std::string trimAscii(const std::string& s) {
    size_t start = 0;
    size_t end = s.size();
    while (start < end && std::isspace(static_cast<unsigned char>(s[start]))) ++start;
    while (end > start && std::isspace(static_cast<unsigned char>(s[end - 1]))) --end;
    return s.substr(start, end - start);
}

struct LogprobsRequest {
    bool enabled{false};
    size_t top_logprobs{0};
};

constexpr size_t kMaxTopLogprobs = 20;

struct ParsedModelName {
    std::string name;
    std::string quantization;
    bool valid{true};
};

ParsedModelName parse_model_name_with_quantization(const std::string& model_name) {
    ParsedModelName parsed;
    parsed.name = model_name;
    const auto pos = model_name.find(':');
    if (pos == std::string::npos) {
        return parsed;
    }
    if (pos == 0 || pos + 1 >= model_name.size()) {
        parsed.valid = false;
        return parsed;
    }
    if (model_name.find(':', pos + 1) != std::string::npos) {
        parsed.valid = false;
        return parsed;
    }
    parsed.name = model_name.substr(0, pos);
    parsed.quantization = model_name.substr(pos + 1);
    return parsed;
}

std::vector<std::string> split_logprob_tokens(const std::string& text) {
    std::vector<std::string> tokens;
    std::string current;
    bool prepend_space = false;
    for (char c : text) {
        if (std::isspace(static_cast<unsigned char>(c))) {
            if (!current.empty()) {
                tokens.push_back(current);
                current.clear();
            }
            prepend_space = true;
        } else {
            if (current.empty() && prepend_space) {
                current.push_back(' ');
                prepend_space = false;
            }
            current.push_back(c);
        }
    }
    if (!current.empty()) {
        tokens.push_back(current);
    }
    return tokens;
}

// T050-T051: Convert real TokenLogprob data to OpenAI-compatible JSON format
json build_logprobs_from_real(const std::vector<TokenLogprob>& logprobs) {
    json tokens_arr = json::array();
    json token_logprobs_arr = json::array();
    json top_logprobs_arr = json::array();

    for (const auto& entry : logprobs) {
        tokens_arr.push_back(entry.token);
        token_logprobs_arr.push_back(entry.logprob);

        json top_entry = json::object();
        for (const auto& [tok, lp] : entry.top_logprobs) {
            top_entry[tok] = lp;
        }
        top_logprobs_arr.push_back(top_entry);
    }

    return json{
        {"tokens", tokens_arr},
        {"token_logprobs", token_logprobs_arr},
        {"top_logprobs", top_logprobs_arr}
    };
}

// T030-T031: Fallback pseudo logprob for cases where real logprobs unavailable
double compute_pseudo_logprob(const std::string& token, size_t position) {
    std::hash<std::string> hasher;
    size_t h = hasher(token) ^ (position * 0x9e3779b9);
    double normalized = static_cast<double>(h % 10000) / 10000.0;
    return -0.01 - (normalized * 4.99);  // -0.01 to -5.0
}

// T030-T034: Fallback logprobs for text-based parsing (used when real logprobs unavailable)
json build_logprobs_fallback(const std::string& text, size_t top_logprobs) {
    const auto tokens = split_logprob_tokens(text);
    json token_logprobs = json::array();
    json top_logprobs_arr = json::array();
    for (size_t i = 0; i < tokens.size(); ++i) {
        const auto& token = tokens[i];
        double logprob = compute_pseudo_logprob(token, i);
        token_logprobs.push_back(logprob);

        json top_entry = json::object();
        if (top_logprobs > 0) {
            top_entry[token] = logprob;
            for (size_t j = 1; j < top_logprobs; ++j) {
                std::string alt_token = "<alt" + std::to_string(j) + ">";
                double alt_logprob = logprob - (static_cast<double>(j) * 0.5);
                top_entry[alt_token] = alt_logprob;
            }
        }
        top_logprobs_arr.push_back(top_entry);
    }
    return json{
        {"tokens", tokens},
        {"token_logprobs", token_logprobs},
        {"top_logprobs", top_logprobs_arr}
    };
}

bool parseLogprobsRequest(const json& body, LogprobsRequest& out, std::string& error) {
    LogprobsRequest req;

    if (body.contains("logprobs")) {
        const auto& value = body["logprobs"];
        if (value.is_boolean()) {
            req.enabled = value.get<bool>();
        } else if (value.is_number_integer()) {
            int v = value.get<int>();
            if (v < 0) {
                error = "logprobs must be >= 0";
                return false;
            }
            req.enabled = v > 0;
            if (v > 0) {
                req.top_logprobs = static_cast<size_t>(v);
            }
        } else if (!value.is_null()) {
            error = "logprobs must be a boolean or integer";
            return false;
        }
    }

    if (body.contains("top_logprobs")) {
        const auto& value = body["top_logprobs"];
        if (!value.is_number_integer()) {
            error = "top_logprobs must be an integer";
            return false;
        }
        int v = value.get<int>();
        if (v < 0) {
            error = "top_logprobs must be >= 0";
            return false;
        }
        req.top_logprobs = static_cast<size_t>(v);
        if (req.top_logprobs > 0) {
            req.enabled = true;
        }
    }

    if (!req.enabled && req.top_logprobs > 0) {
        error = "top_logprobs requires logprobs";
        return false;
    }

    if (req.enabled && req.top_logprobs == 0) {
        req.top_logprobs = 1;
    }

    if (req.top_logprobs > kMaxTopLogprobs) {
        error = "top_logprobs must be <= 20";
        return false;
    }

    out = req;
    return true;
}

bool validateSamplingParams(const nlohmann::json& body, std::string& error) {
    if (body.contains("temperature")) {
        if (!body["temperature"].is_number()) {
            error = "temperature must be a number";
            return false;
        }
        const double v = body["temperature"].get<double>();
        if (v < 0.0 || v > 2.0) {
            error = "temperature must be between 0 and 2";
            return false;
        }
    }
    if (body.contains("top_p")) {
        if (!body["top_p"].is_number()) {
            error = "top_p must be a number";
            return false;
        }
        const double v = body["top_p"].get<double>();
        if (v < 0.0 || v > 1.0) {
            error = "top_p must be between 0 and 1";
            return false;
        }
    }
    if (body.contains("top_k")) {
        if (!body["top_k"].is_number_integer()) {
            error = "top_k must be an integer";
            return false;
        }
        const int v = body["top_k"].get<int>();
        if (v < 0) {
            error = "top_k must be >= 0";
            return false;
        }
    }
    // T027: Validate presence_penalty range (-2.0 to 2.0)
    if (body.contains("presence_penalty")) {
        if (!body["presence_penalty"].is_number()) {
            error = "presence_penalty must be a number";
            return false;
        }
        const double v = body["presence_penalty"].get<double>();
        if (v < -2.0 || v > 2.0) {
            error = "presence_penalty must be between -2 and 2";
            return false;
        }
    }
    // T027: Validate frequency_penalty range (-2.0 to 2.0)
    if (body.contains("frequency_penalty")) {
        if (!body["frequency_penalty"].is_number()) {
            error = "frequency_penalty must be a number";
            return false;
        }
        const double v = body["frequency_penalty"].get<double>();
        if (v < -2.0 || v > 2.0) {
            error = "frequency_penalty must be between -2 and 2";
            return false;
        }
    }
    // T036: Validate n parameter range (1-8)
    if (body.contains("n")) {
        if (!body["n"].is_number_integer()) {
            error = "n must be an integer";
            return false;
        }
        const int v = body["n"].get<int>();
        if (v < 1 || v > 8) {
            error = "n must be between 1 and 8";
            return false;
        }
    }
    if (body.contains("draft_model")) {
        if (!body["draft_model"].is_string()) {
            error = "draft_model must be a string";
            return false;
        }
        const std::string name = body["draft_model"].get<std::string>();
        if (name.empty()) {
            error = "draft_model must not be empty";
            return false;
        }
    }
    if (body.contains("speculative")) {
        const auto& speculative = body["speculative"];
        if (!(speculative.is_boolean() || speculative.is_object())) {
            error = "speculative must be a boolean or object";
            return false;
        }
        if (speculative.is_object()) {
            if (speculative.contains("max_tokens") && !speculative["max_tokens"].is_number_integer()) {
                error = "speculative.max_tokens must be an integer";
                return false;
            }
            if (speculative.contains("min_tokens") && !speculative["min_tokens"].is_number_integer()) {
                error = "speculative.min_tokens must be an integer";
                return false;
            }
            if (speculative.contains("max_tokens")) {
                const int v = speculative["max_tokens"].get<int>();
                if (v < 0) {
                    error = "speculative.max_tokens must be >= 0";
                    return false;
                }
            }
            if (speculative.contains("min_tokens")) {
                const int v = speculative["min_tokens"].get<int>();
                if (v < 0) {
                    error = "speculative.min_tokens must be >= 0";
                    return false;
                }
            }
        }
        if (!body.contains("draft_model")) {
            if (!(speculative.is_boolean() && speculative.get<bool>() == false)) {
                error = "draft_model is required for speculative decoding";
                return false;
            }
        }
    }
    return true;
}

constexpr int kDefaultDraftMaxTokens = 16;

bool parseSpeculativeParams(const nlohmann::json& body, InferenceParams& parsed, std::string& error) {
    if (body.contains("draft_model")) {
        parsed.draft_model = body["draft_model"].get<std::string>();
    }

    if (!body.contains("speculative")) {
        if (!parsed.draft_model.empty() && parsed.draft_max_tokens == 0) {
            parsed.draft_max_tokens = kDefaultDraftMaxTokens;
        }
        return true;
    }

    const auto& speculative = body["speculative"];
    if (speculative.is_boolean()) {
        const bool enabled = speculative.get<bool>();
        if (enabled && parsed.draft_model.empty()) {
            error = "draft_model is required for speculative decoding";
            return false;
        }
        if (enabled && parsed.draft_max_tokens == 0) {
            parsed.draft_max_tokens = kDefaultDraftMaxTokens;
        }
        return true;
    }

    if (!speculative.is_object()) {
        error = "speculative must be a boolean or object";
        return false;
    }

    if (parsed.draft_model.empty()) {
        error = "draft_model is required for speculative decoding";
        return false;
    }

    if (speculative.contains("max_tokens")) {
        parsed.draft_max_tokens = speculative["max_tokens"].get<int>();
    }
    if (speculative.contains("min_tokens")) {
        parsed.draft_min_tokens = speculative["min_tokens"].get<int>();
    }
    if (parsed.draft_max_tokens < 0) {
        error = "speculative.max_tokens must be >= 0";
        return false;
    }
    if (parsed.draft_min_tokens < 0) {
        error = "speculative.min_tokens must be >= 0";
        return false;
    }
    if (parsed.draft_max_tokens == 0) {
        parsed.draft_max_tokens = kDefaultDraftMaxTokens;
    }
    if (parsed.draft_min_tokens > parsed.draft_max_tokens) {
        error = "speculative.min_tokens must be <= speculative.max_tokens";
        return false;
    }
    return true;
}

std::optional<Modelfile> loadModelfile(const std::string& model, std::string& error) {
    return Modelfile::loadForModel(model, error);
}

bool parse_double(const std::string& value, double& out) {
    try {
        size_t idx = 0;
        out = std::stod(value, &idx);
        return idx == value.size();
    } catch (...) {
        return false;
    }
}

bool parse_int(const std::string& value, int& out) {
    try {
        size_t idx = 0;
        long long v = std::stoll(value, &idx);
        if (idx != value.size()) return false;
        if (v < std::numeric_limits<int>::min() || v > std::numeric_limits<int>::max()) return false;
        out = static_cast<int>(v);
        return true;
    } catch (...) {
        return false;
    }
}

std::vector<std::string> parse_stop_sequences(const std::string& value) {
    std::vector<std::string> out;
    std::string trimmed = trimAscii(value);
    if (trimmed.empty()) return out;
    if (trimmed.front() == '[' && trimmed.back() == ']') {
        auto parsed = json::parse(trimmed, nullptr, false);
        if (parsed.is_array()) {
            for (const auto& item : parsed) {
                if (item.is_string()) {
                    out.push_back(item.get<std::string>());
                }
            }
            return out;
        }
    }
    std::stringstream ss(trimmed);
    std::string token;
    while (std::getline(ss, token, ',')) {
        token = trimAscii(token);
        if (!token.empty()) out.push_back(token);
    }
    if (out.empty() && !trimmed.empty()) {
        out.push_back(trimmed);
    }
    return out;
}

bool applyModelfileDefaults(const nlohmann::json& body,
                            const Modelfile& modelfile,
                            InferenceParams& params,
                            std::vector<ChatMessage>& messages,
                            std::string& error) {
    if (!modelfile.system_prompt.empty()) {
        messages.insert(messages.begin(), ChatMessage{"system", modelfile.system_prompt});
    }
    if (!modelfile.messages.empty()) {
        messages.insert(messages.begin() + (modelfile.system_prompt.empty() ? 0 : 1),
                        modelfile.messages.begin(),
                        modelfile.messages.end());
    }
    if (!modelfile.template_text.empty() && params.chat_template.empty()) {
        params.chat_template = modelfile.template_text;
    }

    for (const auto& [key, value] : modelfile.parameters) {
        if (key == "temperature") {
            if (body.contains("temperature")) continue;
            double v;
            if (!parse_double(value, v)) {
                error = "Modelfile PARAMETER temperature is invalid";
                return false;
            }
            params.temperature = static_cast<float>(v);
        } else if (key == "top_p") {
            if (body.contains("top_p")) continue;
            double v;
            if (!parse_double(value, v)) {
                error = "Modelfile PARAMETER top_p is invalid";
                return false;
            }
            params.top_p = static_cast<float>(v);
        } else if (key == "top_k") {
            if (body.contains("top_k")) continue;
            int v;
            if (!parse_int(value, v)) {
                error = "Modelfile PARAMETER top_k is invalid";
                return false;
            }
            params.top_k = v;
        } else if (key == "repeat_penalty") {
            if (body.contains("repeat_penalty")) continue;
            double v;
            if (!parse_double(value, v)) {
                error = "Modelfile PARAMETER repeat_penalty is invalid";
                return false;
            }
            params.repeat_penalty = static_cast<float>(v);
        } else if (key == "presence_penalty") {
            if (body.contains("presence_penalty")) continue;
            double v;
            if (!parse_double(value, v)) {
                error = "Modelfile PARAMETER presence_penalty is invalid";
                return false;
            }
            params.presence_penalty = static_cast<float>(v);
        } else if (key == "frequency_penalty") {
            if (body.contains("frequency_penalty")) continue;
            double v;
            if (!parse_double(value, v)) {
                error = "Modelfile PARAMETER frequency_penalty is invalid";
                return false;
            }
            params.frequency_penalty = static_cast<float>(v);
        } else if (key == "num_predict" || key == "max_tokens") {
            if (body.contains("max_tokens")) continue;
            int v;
            if (!parse_int(value, v)) {
                error = "Modelfile PARAMETER max_tokens is invalid";
                return false;
            }
            if (v > 0) params.max_tokens = static_cast<size_t>(v);
        } else if (key == "seed") {
            if (body.contains("seed")) continue;
            int v;
            if (!parse_int(value, v)) {
                error = "Modelfile PARAMETER seed is invalid";
                return false;
            }
            if (v > 0) params.seed = static_cast<uint32_t>(v);
        } else if (key == "stop") {
            if (body.contains("stop")) continue;
            auto sequences = parse_stop_sequences(value);
            if (sequences.empty()) {
                error = "Modelfile PARAMETER stop is invalid";
                return false;
            }
            params.stop_sequences = std::move(sequences);
        } else if (key == "draft_model") {
            if (body.contains("draft_model")) continue;
            params.draft_model = value;
        }
    }
    return true;
}

bool parseStopSequences(const nlohmann::json& body, std::vector<std::string>& out, std::string& error) {
    if (!body.contains("stop")) return true;
    const auto& stop = body["stop"];
    if (stop.is_null()) return true;

    if (stop.is_string()) {
        std::string seq = stop.get<std::string>();
        if (seq.empty()) {
            error = "stop must not be empty";
            return false;
        }
        out.push_back(std::move(seq));
        return true;
    }

    if (stop.is_array()) {
        for (const auto& item : stop) {
            if (!item.is_string()) {
                error = "stop must be a string or array of strings";
                return false;
            }
            std::string seq = item.get<std::string>();
            if (seq.empty()) {
                error = "stop sequences must not be empty";
                return false;
            }
            out.push_back(std::move(seq));
        }
        return true;
    }

    error = "stop must be a string or array of strings";
    return false;
}

bool parseLoraEntry(const nlohmann::json& entry,
                    std::vector<LoraRequest>& out,
                    std::string& error) {
    if (entry.is_string()) {
        LoraRequest req;
        std::string value = entry.get<std::string>();
        if (value.empty()) {
            error = "lora name must not be empty";
            return false;
        }
        if (std::filesystem::path(value).is_absolute()) {
            req.path = value;
            req.name = std::filesystem::path(value).filename().string();
        } else {
            req.name = value;
        }
        out.push_back(std::move(req));
        return true;
    }

    if (entry.is_object()) {
        LoraRequest req;
        if (entry.contains("path")) {
            if (!entry["path"].is_string()) {
                error = "lora.path must be a string";
                return false;
            }
            req.path = entry["path"].get<std::string>();
        }
        if (entry.contains("lora")) {
            if (!entry["lora"].is_string()) {
                error = "lora.lora must be a string";
                return false;
            }
            req.name = entry["lora"].get<std::string>();
        } else if (entry.contains("name")) {
            if (!entry["name"].is_string()) {
                error = "lora.name must be a string";
                return false;
            }
            req.name = entry["name"].get<std::string>();
        }
        if (entry.contains("scale")) {
            if (!entry["scale"].is_number()) {
                error = "lora.scale must be a number";
                return false;
            }
            req.scale = entry["scale"].get<float>();
        }
        if (req.name.empty() && req.path.empty()) {
            error = "lora entry must include name or path";
            return false;
        }
        out.push_back(std::move(req));
        return true;
    }

    error = "lora must be a string, object, or array";
    return false;
}

bool parseLoraParams(const nlohmann::json& body,
                     std::vector<LoraRequest>& out,
                     std::string& error) {
    if (!body.contains("lora")) {
        return true;
    }
    const auto& lora = body["lora"];
    if (lora.is_array()) {
        for (const auto& entry : lora) {
            if (!parseLoraEntry(entry, out, error)) {
                return false;
            }
        }
        return true;
    }
    return parseLoraEntry(lora, out, error);
}

bool parseInferenceParams(const nlohmann::json& body, InferenceParams& params, std::string& error) {
    InferenceParams parsed;

    // OpenAI-compatible fields
    if (body.contains("max_tokens") && body["max_tokens"].is_number_integer()) {
        int v = body["max_tokens"].get<int>();
        if (v > 0) parsed.max_tokens = static_cast<size_t>(v);
    }
    if (body.contains("temperature") && body["temperature"].is_number()) {
        parsed.temperature = body["temperature"].get<float>();
    }
    if (body.contains("top_p") && body["top_p"].is_number()) {
        parsed.top_p = body["top_p"].get<float>();
    }
    if (body.contains("top_k") && body["top_k"].is_number_integer()) {
        parsed.top_k = body["top_k"].get<int>();
    }
    if (body.contains("repeat_penalty") && body["repeat_penalty"].is_number()) {
        parsed.repeat_penalty = body["repeat_penalty"].get<float>();
    }
    if (body.contains("seed") && body["seed"].is_number_integer()) {
        int64_t v = body["seed"].get<int64_t>();
        if (v > 0 && v <= static_cast<int64_t>(std::numeric_limits<uint32_t>::max())) {
            parsed.seed = static_cast<uint32_t>(v);
        }
    }

    if (body.contains("draft_model") && body["draft_model"].is_string()) {
        parsed.draft_model = body["draft_model"].get<std::string>();
    }

    // T025-T026: Parse presence_penalty and frequency_penalty
    if (body.contains("presence_penalty") && body["presence_penalty"].is_number()) {
        parsed.presence_penalty = body["presence_penalty"].get<float>();
    }
    if (body.contains("frequency_penalty") && body["frequency_penalty"].is_number()) {
        parsed.frequency_penalty = body["frequency_penalty"].get<float>();
    }

    // T035: Parse n parameter
    if (body.contains("n") && body["n"].is_number_integer()) {
        parsed.n = body["n"].get<int>();
    }

    if (!parseSpeculativeParams(body, parsed, error)) {
        return false;
    }

    // Parse logprobs settings
    if (body.contains("logprobs")) {
        const auto& value = body["logprobs"];
        if (value.is_boolean()) {
            parsed.logprobs = value.get<bool>();
        } else if (value.is_number_integer() && value.get<int>() > 0) {
            parsed.logprobs = true;
        }
    }
    if (body.contains("top_logprobs") && body["top_logprobs"].is_number_integer()) {
        parsed.top_logprobs = body["top_logprobs"].get<int>();
        if (parsed.top_logprobs > 0) {
            parsed.logprobs = true;
        }
    }

    if (!parseStopSequences(body, parsed.stop_sequences, error)) {
        return false;
    }

    if (!parseLoraParams(body, parsed.loras, error)) {
        return false;
    }

    // Parse tools / function calling
    auto parse_tool_entry = [&](const json& function, std::vector<ToolDefinition>& out) -> bool {
        if (!function.is_object()) {
            error = "function definition must be an object";
            return false;
        }
        if (!function.contains("name") || !function["name"].is_string()) {
            error = "function name is required";
            return false;
        }
        ToolDefinition tool;
        tool.name = function["name"].get<std::string>();
        if (function.contains("description") && function["description"].is_string()) {
            tool.description = function["description"].get<std::string>();
        }
        if (function.contains("parameters")) {
            if (function["parameters"].is_object()) {
                tool.parameters_json = function["parameters"].dump();
            } else if (function["parameters"].is_string()) {
                tool.parameters_json = function["parameters"].get<std::string>();
            } else {
                error = "function parameters must be an object or string";
                return false;
            }
        }
        out.push_back(std::move(tool));
        return true;
    };

    if (body.contains("tools")) {
        if (!body["tools"].is_array()) {
            error = "tools must be an array";
            return false;
        }
        for (const auto& tool : body["tools"]) {
            if (!tool.is_object()) {
                error = "tools entry must be an object";
                return false;
            }
            std::string type = tool.value("type", "function");
            if (type != "function") {
                error = "only function tools are supported";
                return false;
            }
            if (!tool.contains("function")) {
                error = "tool.function is required";
                return false;
            }
            if (!parse_tool_entry(tool["function"], parsed.tools)) {
                return false;
            }
        }
    }

    if (body.contains("functions")) {
        if (!body["functions"].is_array()) {
            error = "functions must be an array";
            return false;
        }
        for (const auto& func : body["functions"]) {
            if (!parse_tool_entry(func, parsed.tools)) {
                return false;
            }
        }
    }

    if (body.contains("tool_choice")) {
        const auto& choice = body["tool_choice"];
        if (choice.is_string()) {
            std::string mode = choice.get<std::string>();
            if (mode == "none") {
                parsed.tools.clear();
                parsed.forced_tool_name.clear();
            } else if (mode != "auto") {
                error = "tool_choice must be 'auto', 'none', or an object";
                return false;
            }
        } else if (choice.is_object()) {
            std::string type = choice.value("type", "function");
            if (type != "function" || !choice.contains("function") ||
                !choice["function"].is_object() ||
                !choice["function"].contains("name") ||
                !choice["function"]["name"].is_string()) {
                error = "tool_choice.function.name is required";
                return false;
            }
            parsed.forced_tool_name = choice["function"]["name"].get<std::string>();
        } else {
            error = "tool_choice must be a string or object";
            return false;
        }
    }

    if (!parsed.tools.empty()) {
        parsed.grammar = get_json_grammar();
    }

    params = std::move(parsed);
    return true;
}

std::string applyStopSequences(std::string output, const std::vector<std::string>& stops) {
    if (stops.empty()) return output;
    size_t earliest = std::string::npos;
    for (const auto& stop : stops) {
        if (stop.empty()) continue;
        size_t pos = output.find(stop);
        if (pos != std::string::npos && (earliest == std::string::npos || pos < earliest)) {
            earliest = pos;
        }
    }
    if (earliest == std::string::npos) return output;
    output.resize(earliest);
    return output;
}
}  // namespace

// T017: Generate unique response ID with prefix, timestamp, and random component
std::string generate_response_id(const std::string& prefix) {
    auto now = std::chrono::system_clock::now();
    auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
        now.time_since_epoch()).count();

    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> dis(0, 0xFFFF);

    std::ostringstream oss;
    oss << prefix << "-" << std::hex << ms << "-"
        << std::setw(4) << std::setfill('0') << dis(gen);
    return oss.str();
}

// T018: Get current Unix timestamp in seconds
int64_t get_current_timestamp() {
    auto now = std::chrono::system_clock::now();
    return std::chrono::duration_cast<std::chrono::seconds>(
        now.time_since_epoch()).count();
}

struct ParsedChatMessages {
    std::vector<ChatMessage> messages;
    std::vector<std::string> image_urls;
};

constexpr size_t kMaxImageCount = 10;
const std::string kVisionMarker = "<__media__>";

bool parseChatMessages(const json& body, ParsedChatMessages& out, std::string& error) {
    out.messages.clear();
    out.image_urls.clear();

    if (!body.contains("messages")) {
        return true;
    }
    if (!body["messages"].is_array()) {
        error = "messages must be an array";
        return false;
    }

    for (const auto& m : body["messages"]) {
        if (!m.is_object()) {
            error = "message must be an object";
            return false;
        }
        std::string role = m.value("role", "");
        if (role.empty()) {
            error = "message.role is required";
            return false;
        }

        std::string content;
        if (!m.contains("content") || m["content"].is_null()) {
            out.messages.push_back({role, ""});
            continue;
        }

        const auto& c = m["content"];
        if (c.is_string()) {
            content = c.get<std::string>();
        } else if (c.is_array()) {
            for (const auto& part : c) {
                if (!part.is_object()) {
                    error = "content part must be an object";
                    return false;
                }
                std::string type = part.value("type", "");
                if (type == "text") {
                    content += part.value("text", "");
                } else if (type == "image_url") {
                    std::string url;
                    if (part.contains("image_url")) {
                        const auto& image_url = part["image_url"];
                        if (image_url.is_object()) {
                            url = image_url.value("url", "");
                        } else if (image_url.is_string()) {
                            url = image_url.get<std::string>();
                        }
                    }
                    if (url.empty()) {
                        error = "image_url.url is required";
                        return false;
                    }
                    out.image_urls.push_back(url);
                    if (out.image_urls.size() > kMaxImageCount) {
                        error = "too many images in request";
                        return false;
                    }
                    content += kVisionMarker;
                } else {
                    error = "unsupported content type: " + type;
                    return false;
                }
            }
        } else {
            error = "content must be a string or array";
            return false;
        }

        out.messages.push_back({role, content});
    }

    return true;
}

struct ParsedResponsesInput {
    std::vector<ChatMessage> messages;
    std::vector<std::string> image_urls;
};

bool parseResponsesInput(const json& body, ParsedResponsesInput& out, std::string& error) {
    out.messages.clear();
    out.image_urls.clear();

    if (!body.contains("input")) {
        error = "input is required";
        return false;
    }

    auto parse_content = [&](const json& content, std::string& content_out) -> bool {
        if (content.is_null()) {
            return true;
        }
        if (content.is_string()) {
            content_out += content.get<std::string>();
            return true;
        }
        if (!content.is_array()) {
            error = "content must be a string or array";
            return false;
        }

        for (const auto& part : content) {
            if (!part.is_object()) {
                error = "content part must be an object";
                return false;
            }
            std::string type = part.value("type", "");
            if (type == "text" || type == "input_text") {
                content_out += part.value("text", "");
                continue;
            }
            if (type == "image_url" || type == "input_image") {
                std::string url;
                if (part.contains("image_url")) {
                    const auto& image_url = part["image_url"];
                    if (image_url.is_object()) {
                        url = image_url.value("url", "");
                    } else if (image_url.is_string()) {
                        url = image_url.get<std::string>();
                    }
                } else if (part.contains("url") && part["url"].is_string()) {
                    url = part["url"].get<std::string>();
                }
                if (url.empty()) {
                    error = "image_url.url is required";
                    return false;
                }
                out.image_urls.push_back(url);
                if (out.image_urls.size() > kMaxImageCount) {
                    error = "too many images in request";
                    return false;
                }
                content_out += kVisionMarker;
                continue;
            }
            error = "unsupported content type: " + type;
            return false;
        }
        return true;
    };

    auto add_message = [&](const std::string& role, const json& content) -> bool {
        std::string text;
        if (!parse_content(content, text)) {
            return false;
        }
        out.messages.push_back({role, text});
        return true;
    };

    const auto& input = body["input"];
    if (input.is_string()) {
        std::string text = input.get<std::string>();
        if (trimAscii(text).empty()) {
            error = "input must not be empty";
            return false;
        }
        out.messages.push_back({"user", text});
        return true;
    }

    json input_array = input;
    if (input.is_object()) {
        input_array = json::array({input});
    }
    if (!input_array.is_array() || input_array.empty()) {
        error = "input must be a string or array";
        return false;
    }

    if (input_array[0].is_string()) {
        std::string merged;
        for (const auto& entry : input_array) {
            if (!entry.is_string()) {
                error = "input array must contain only strings";
                return false;
            }
            if (!merged.empty()) merged += "\n";
            merged += entry.get<std::string>();
        }
        if (trimAscii(merged).empty()) {
            error = "input must not be empty";
            return false;
        }
        out.messages.push_back({"user", merged});
        return true;
    }

    if (!input_array[0].is_object()) {
        error = "input array must contain objects";
        return false;
    }

    const bool looks_like_messages = input_array[0].contains("role");
    const bool looks_like_parts = input_array[0].contains("type");
    if (looks_like_messages) {
        for (const auto& msg : input_array) {
            if (!msg.is_object()) {
                error = "message must be an object";
                return false;
            }
            std::string role = msg.value("role", "");
            if (role.empty()) {
                error = "message.role is required";
                return false;
            }
            if (!msg.contains("content") || msg["content"].is_null()) {
                out.messages.push_back({role, ""});
                continue;
            }
            if (!add_message(role, msg["content"])) {
                return false;
            }
        }
        return true;
    }

    if (looks_like_parts) {
        json parts_wrapper = json::array();
        for (const auto& part : input_array) {
            parts_wrapper.push_back(part);
        }
        return add_message("user", parts_wrapper);
    }

    error = "input array must contain messages or content parts";
    return false;
}

OpenAIEndpoints::OpenAIEndpoints(ModelRegistry& registry, InferenceEngine& engine, const NodeConfig& config, GpuBackend backend)
    : registry_(registry), engine_(engine), config_(config), backend_(backend) {}

void OpenAIEndpoints::registerRoutes(httplib::Server& server) {
    server.Get("/v1/models", [this](const httplib::Request&, httplib::Response& res) {
        json body;
        body["object"] = "list";
        body["data"] = json::array();
        for (const auto& id : registry_.listExecutableModels()) {
            bool has_vision = registry_.hasVisionCapability(id);
            json model_entry = {
                {"id", id},
                {"object", "model"},
                {"owned_by", "load balancer"},
                {"capabilities", {
                    {"chat_completion", true},
                    {"completion", true},
                    {"embeddings", false},
                    {"fine_tune", false},
                    {"image_generation", false},
                    {"image_understanding", has_vision},
                    {"inference", true},
                    {"speech_to_text", false},
                    {"text_to_speech", false}
                }}
            };
            body["data"].push_back(model_entry);
        }
        setJson(res, body);
    });

    server.Post("/v1/chat/completions", [this](const httplib::Request& req, httplib::Response& res) {
        if (!checkReady(res)) return;
        auto guard = RequestGuard::acquire_with_timeout(get_request_queue_timeout());
        if (!guard) {
            respondError(res, 429, "too_many_requests", "Node is busy");
            return;
        }
        try {
            auto body = json::parse(req.body);
            std::string model = body.value("model", "");
            if (!validateModel(model, "text", res)) return;
            ParsedChatMessages parsed;
            std::string parse_error;
            if (!parseChatMessages(body, parsed, parse_error)) {
                respondError(res, 400, "bad_request", parse_error);
                return;
            }
            std::string param_error;
            if (!validateSamplingParams(body, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }
            bool has_prompt = false;
            for (const auto& msg : parsed.messages) {
                if (!trimAscii(msg.content).empty()) {
                    has_prompt = true;
                    break;
                }
            }
            if (!has_prompt) {
                respondError(res, 400, "invalid_request", "prompt must not be empty");
                return;
            }
            bool stream = body.value("stream", false);
            InferenceParams params;
            if (!parseInferenceParams(body, params, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }
            std::string modelfile_error;
            auto modelfile = loadModelfile(model, modelfile_error);
            if (!modelfile_error.empty() && !modelfile) {
                respondError(res, 400, "invalid_request", modelfile_error);
                return;
            }
            if (!params.draft_model.empty()) {
                if (!validateModel(params.draft_model, "text", res)) return;
            }
            LogprobsRequest logprobs_req;
            if (!parseLogprobsRequest(body, logprobs_req, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }
            std::vector<ChatMessage> messages_for_generation = parsed.messages;
            if (modelfile) {
                if (!applyModelfileDefaults(body, *modelfile, params, messages_for_generation, modelfile_error)) {
                    respondError(res, 400, "invalid_request", modelfile_error);
                    return;
                }
            }
            if (!params.draft_model.empty()) {
                if (!validateModel(params.draft_model, "text", res)) return;
            }
            if (!params.tools.empty()) {
                std::string tool_prompt = formatToolsForPrompt(params.tools);
                if (!tool_prompt.empty()) {
                    messages_for_generation.insert(
                        messages_for_generation.begin(),
                        ChatMessage{"system", tool_prompt});
                }
            }

            if (stream) {
                if (logprobs_req.enabled) {
                    respondError(res, 400, "invalid_request", "logprobs is not supported with stream");
                    return;
                }
                // T041: n > 1 とストリーミングの同時指定は非対応
                if (params.n > 1) {
                    respondError(res, 400, "invalid_request", "n > 1 is not supported with stream");
                    return;
                }
                // ストリーミング用に生成
                std::string output;
                if (!parsed.image_urls.empty()) {
                    output = engine_.generateChatWithImages(messages_for_generation, parsed.image_urls, model, params);
                } else {
                    output = engine_.generateChat(messages_for_generation, model, params);
                }
                output = applyStopSequences(std::move(output), params.stop_sequences);
                output = sanitize_utf8_lossy(output);

                auto guard_ptr = std::make_shared<RequestGuard>(std::move(*guard));
                // T039: Generate stream ID and timestamp once for all chunks
                std::string stream_id = generate_response_id("chatcmpl");
                int64_t stream_created = get_current_timestamp();
                res.set_header("Content-Type", "text/event-stream");
                res.set_chunked_content_provider("text/event-stream",
                    [output, model, stream_id, stream_created, guard_ptr](size_t offset, httplib::DataSink& sink) {
                        if (offset == 0) {
                            // OpenAI compatible streaming format
                            json event_data = {
                                {"id", stream_id},
                                {"object", "chat.completion.chunk"},
                                {"created", stream_created},
                                {"model", model},
                                {"choices", json::array({{
                                    {"index", 0},
                                    {"delta", {{"content", output}}},
                                    {"finish_reason", nullptr}
                                }})}
                            };
                            std::string chunk = "data: " + event_data.dump() + "\n\n";
                            sink.write(chunk.data(), chunk.size());
                            std::string done = "data: [DONE]\n\n";
                            sink.write(done.data(), done.size());
                            sink.done();
                        }
                        return true;
                    });
                return;
            }

            // T019: Build prompt from messages for token counting
            std::string prompt_text;
            for (const auto& msg : messages_for_generation) {
                prompt_text += msg.role + ": " + msg.content + "\n";
            }
            int prompt_tokens = static_cast<int>(prompt_text.length() / 4);  // Approximate tokenization

            // T037: n回の生成ループでchoices配列を構築
            json choices = json::array();
            int total_completion_tokens = 0;
            bool tool_choice_failed = false;
            std::string tool_choice_error;
            for (int i = 0; i < params.n; ++i) {
                // T050: Prepare logprobs output buffer if requested
                std::vector<TokenLogprob> logprobs_out;
                if (logprobs_req.enabled) {
                    params.logprobs = true;
                    params.top_logprobs = static_cast<int>(logprobs_req.top_logprobs);
                    params.out_logprobs = &logprobs_out;
                }

                std::string gen_output;
                if (!parsed.image_urls.empty()) {
                    gen_output = engine_.generateChatWithImages(messages_for_generation, parsed.image_urls, model, params);
                } else {
                    gen_output = engine_.generateChat(messages_for_generation, model, params);
                }
                gen_output = applyStopSequences(std::move(gen_output), params.stop_sequences);
                gen_output = sanitize_utf8_lossy(gen_output);

                json choice = {
                    {"index", i},
                    {"message", {{"role", "assistant"}, {"content", gen_output}}},
                    {"finish_reason", "stop"}
                };

                if (!params.tools.empty()) {
                    auto tool_calls = detectToolCalls(gen_output);
                    if (!params.forced_tool_name.empty()) {
                        std::vector<ToolCall> filtered;
                        for (const auto& call : tool_calls) {
                            if (call.function_name == params.forced_tool_name) {
                                filtered.push_back(call);
                            }
                        }
                        tool_calls = std::move(filtered);
                        if (tool_calls.empty()) {
                            tool_choice_failed = true;
                            tool_choice_error = "model did not call required tool";
                            break;
                        }
                    }

                    if (!tool_calls.empty()) {
                        json tool_calls_json = json::array();
                        for (const auto& call : tool_calls) {
                            tool_calls_json.push_back({
                                {"id", call.id},
                                {"type", "function"},
                                {"function", {
                                    {"name", call.function_name},
                                    {"arguments", call.arguments_json}
                                }}
                            });
                        }
                        choice["message"]["content"] = nullptr;
                        choice["message"]["tool_calls"] = tool_calls_json;
                        choice["finish_reason"] = "tool_calls";
                    }
                }
                if (logprobs_req.enabled) {
                    // T050-T051: Use real logprobs if available, otherwise fallback
                    if (!logprobs_out.empty()) {
                        choice["logprobs"] = build_logprobs_from_real(logprobs_out);
                    } else {
                        choice["logprobs"] = build_logprobs_fallback(gen_output, logprobs_req.top_logprobs);
                    }
                }
                choices.push_back(choice);
                total_completion_tokens += static_cast<int>(gen_output.length() / 4);
            }

            if (tool_choice_failed) {
                respondError(res, 400, "invalid_response", tool_choice_error);
                return;
            }

            // T019, T021, T023: Add usage, dynamic ID, and current timestamp
            json resp = {
                {"id", generate_response_id("chatcmpl")},
                {"object", "chat.completion"},
                {"created", get_current_timestamp()},
                {"model", model},
                {"choices", choices},
                {"usage", {
                    {"prompt_tokens", prompt_tokens},
                    {"completion_tokens", total_completion_tokens},
                    {"total_tokens", prompt_tokens + total_completion_tokens}
                }}
            };
            setJson(res, resp);
        } catch (const std::exception& e) {
            respondError(res, 400, "bad_request", std::string("error: ") + e.what());
        } catch (...) {
            respondError(res, 400, "bad_request", "invalid JSON body");
        }
    });

    server.Post("/v1/responses", [this](const httplib::Request& req, httplib::Response& res) {
        if (!checkReady(res)) return;
        auto guard = RequestGuard::acquire_with_timeout(get_request_queue_timeout());
        if (!guard) {
            respondError(res, 429, "too_many_requests", "Node is busy");
            return;
        }
        try {
            auto body = json::parse(req.body);
            std::string model = body.value("model", "");
            if (!validateModel(model, "text", res)) return;

            ParsedResponsesInput parsed;
            std::string parse_error;
            if (!parseResponsesInput(body, parsed, parse_error)) {
                respondError(res, 400, "invalid_request", parse_error);
                return;
            }

            std::string instructions = body.value("instructions", "");
            if (!instructions.empty()) {
                parsed.messages.insert(parsed.messages.begin(), {"system", instructions});
            }

            std::string param_error;
            if (!validateSamplingParams(body, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }
            InferenceParams params;
            if (!parseInferenceParams(body, params, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }
            std::string modelfile_error;
            auto modelfile = loadModelfile(model, modelfile_error);
            if (!modelfile_error.empty() && !modelfile) {
                respondError(res, 400, "invalid_request", modelfile_error);
                return;
            }
            if (!modelfile) {
                modelfile_error.clear();
            }
            if (!params.draft_model.empty()) {
                if (!validateModel(params.draft_model, "text", res)) return;
            }
            if (body.contains("max_output_tokens")) {
                if (!body["max_output_tokens"].is_number_integer()) {
                    respondError(res, 400, "invalid_request", "max_output_tokens must be an integer");
                    return;
                }
                int max_output_tokens = body["max_output_tokens"].get<int>();
                if (max_output_tokens > 0) {
                    params.max_tokens = static_cast<size_t>(max_output_tokens);
                }
            }

            std::string prompt_text;
            std::vector<ChatMessage> messages_for_generation = parsed.messages;
            if (modelfile) {
                if (!applyModelfileDefaults(body, *modelfile, params, messages_for_generation, modelfile_error)) {
                    respondError(res, 400, "invalid_request", modelfile_error);
                    return;
                }
            }
            if (!params.draft_model.empty()) {
                if (!validateModel(params.draft_model, "text", res)) return;
            }
            for (const auto& msg : messages_for_generation) {
                prompt_text += msg.role + ": " + msg.content + "\n";
            }
            int input_tokens = static_cast<int>(prompt_text.length() / 4);

            bool stream = body.value("stream", false);
            if (stream) {
                auto guard_ptr = std::make_shared<RequestGuard>(std::move(*guard));
                std::string output;
                if (!parsed.image_urls.empty()) {
                    output = engine_.generateChatWithImages(messages_for_generation, parsed.image_urls, model, params);
                } else {
                    output = engine_.generateChat(messages_for_generation, model, params);
                }
                output = applyStopSequences(std::move(output), params.stop_sequences);
                output = sanitize_utf8_lossy(output);

                int output_tokens = static_cast<int>(output.length() / 4);
                json response = {
                    {"id", generate_response_id("resp")},
                    {"object", "response"},
                    {"created_at", get_current_timestamp()},
                    {"model", model},
                    {"output", json::array({{
                        {"type", "message"},
                        {"role", "assistant"},
                        {"content", json::array({{
                            {"type", "output_text"},
                            {"text", output}
                        }})}
                    }})},
                    {"usage", {
                        {"input_tokens", input_tokens},
                        {"output_tokens", output_tokens},
                        {"total_tokens", input_tokens + output_tokens}
                    }}
                };

                json delta = {
                    {"type", "response.output_text.delta"},
                    {"delta", output}
                };
                json completed = {
                    {"type", "response.completed"},
                    {"response", response}
                };

                res.set_chunked_content_provider("text/event-stream",
                    [delta, completed, guard_ptr](size_t offset, httplib::DataSink& sink) {
                        if (offset == 0) {
                            std::string chunk = "event: response.output_text.delta\n";
                            chunk += "data: " + delta.dump() + "\n\n";
                            sink.write(chunk.data(), chunk.size());
                            std::string done = "event: response.completed\n";
                            done += "data: " + completed.dump() + "\n\n";
                            sink.write(done.data(), done.size());
                            sink.done();
                        }
                        return true;
                    });
                return;
            }

            std::string output;
            if (!parsed.image_urls.empty()) {
                output = engine_.generateChatWithImages(parsed.messages, parsed.image_urls, model, params);
            } else {
                output = engine_.generateChat(parsed.messages, model, params);
            }
            output = applyStopSequences(std::move(output), params.stop_sequences);
            output = sanitize_utf8_lossy(output);

            int output_tokens = static_cast<int>(output.length() / 4);
            json resp = {
                {"id", generate_response_id("resp")},
                {"object", "response"},
                {"created_at", get_current_timestamp()},
                {"model", model},
                {"output", json::array({{
                    {"type", "message"},
                    {"role", "assistant"},
                    {"content", json::array({{
                        {"type", "output_text"},
                        {"text", output}
                    }})}
                }})},
                {"usage", {
                    {"input_tokens", input_tokens},
                    {"output_tokens", output_tokens},
                    {"total_tokens", input_tokens + output_tokens}
                }}
            };
            setJson(res, resp);
        } catch (const std::exception& e) {
            respondError(res, 400, "bad_request", std::string("error: ") + e.what());
        } catch (...) {
            respondError(res, 400, "bad_request", "invalid JSON body");
        }
    });

    server.Post("/v1/completions", [this](const httplib::Request& req, httplib::Response& res) {
        if (!checkReady(res)) return;
        auto guard = RequestGuard::acquire_with_timeout(get_request_queue_timeout());
        if (!guard) {
            respondError(res, 429, "too_many_requests", "Node is busy");
            return;
        }
        try {
            auto body = json::parse(req.body);
            std::string model = body.value("model", "");
            if (!validateModel(model, "text", res)) return;
            if (!body.contains("prompt") || !body["prompt"].is_string()) {
                respondError(res, 400, "invalid_request", "prompt is required");
                return;
            }
            std::string prompt = body["prompt"].get<std::string>();
            if (trimAscii(prompt).empty()) {
                respondError(res, 400, "invalid_request", "prompt must not be empty");
                return;
            }
            std::string param_error;
            if (!validateSamplingParams(body, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }
            InferenceParams params;
            if (!parseInferenceParams(body, params, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }
            std::string modelfile_error;
            auto modelfile = loadModelfile(model, modelfile_error);
            if (!modelfile_error.empty() && !modelfile) {
                respondError(res, 400, "invalid_request", modelfile_error);
                return;
            }
            std::vector<ChatMessage> dummy_messages;
            if (modelfile) {
                if (!applyModelfileDefaults(body, *modelfile, params, dummy_messages, modelfile_error)) {
                    respondError(res, 400, "invalid_request", modelfile_error);
                    return;
                }
            }
            if (!params.draft_model.empty()) {
                if (!validateModel(params.draft_model, "text", res)) return;
            }
            LogprobsRequest logprobs_req;
            if (!parseLogprobsRequest(body, logprobs_req, param_error)) {
                respondError(res, 400, "invalid_request", param_error);
                return;
            }

            // T020, T022, T024: Add usage, dynamic ID, and current timestamp
            int prompt_tokens = static_cast<int>(prompt.length() / 4);  // Approximate tokenization

            // T038: n回の生成ループでchoices配列を構築
            json choices = json::array();
            int total_completion_tokens = 0;
            for (int i = 0; i < params.n; ++i) {
                // T050: Prepare logprobs output buffer if requested
                std::vector<TokenLogprob> logprobs_out;
                if (logprobs_req.enabled) {
                    params.logprobs = true;
                    params.top_logprobs = static_cast<int>(logprobs_req.top_logprobs);
                    params.out_logprobs = &logprobs_out;
                }

                std::string output = engine_.generateCompletion(prompt, model, params);
                output = applyStopSequences(std::move(output), params.stop_sequences);
                output = sanitize_utf8_lossy(output);

                json choice = {
                    {"text", output},
                    {"index", i},
                    {"finish_reason", "stop"}
                };
                if (logprobs_req.enabled) {
                    // T050-T051: Use real logprobs if available, otherwise fallback
                    if (!logprobs_out.empty()) {
                        choice["logprobs"] = build_logprobs_from_real(logprobs_out);
                    } else {
                        choice["logprobs"] = build_logprobs_fallback(output, logprobs_req.top_logprobs);
                    }
                }
                choices.push_back(choice);
                total_completion_tokens += static_cast<int>(output.length() / 4);
            }

            json resp = {
                {"id", generate_response_id("cmpl")},
                {"object", "text_completion"},
                {"created", get_current_timestamp()},
                {"model", model},
                {"choices", choices},
                {"usage", {
                    {"prompt_tokens", prompt_tokens},
                    {"completion_tokens", total_completion_tokens},
                    {"total_tokens", prompt_tokens + total_completion_tokens}
                }}
            };
            setJson(res, resp);
        } catch (...) {
            respondError(res, 400, "bad_request", "invalid JSON body");
        }
    });

    server.Post("/v1/embeddings", [this](const httplib::Request& req, httplib::Response& res) {
        if (!checkReady(res)) return;
        auto guard = RequestGuard::acquire_with_timeout(get_request_queue_timeout());
        if (!guard) {
            respondError(res, 429, "too_many_requests", "Node is busy");
            return;
        }
        try {
            auto body = json::parse(req.body);
            // モデルパラメータは必須（OpenAI API仕様準拠）
            if (!body.contains("model") || !body["model"].is_string() || body["model"].get<std::string>().empty()) {
                respondError(res, 400, "invalid_request", "model is required");
                return;
            }
            std::string model = body["model"].get<std::string>();
            if (!validateModel(model, "embeddings", res)) return;

            // inputを解析（文字列または文字列の配列）
            std::vector<std::string> inputs;
            if (body.contains("input")) {
                if (body["input"].is_string()) {
                    inputs.push_back(body["input"].get<std::string>());
                } else if (body["input"].is_array()) {
                    for (const auto& item : body["input"]) {
                        if (item.is_string()) {
                            inputs.push_back(item.get<std::string>());
                        }
                    }
                }
            }

            if (inputs.empty()) {
                respondError(res, 400, "invalid_request", "input is required");
                return;
            }

            // embeddingを生成
            auto embeddings = engine_.generateEmbeddings(inputs, model);

            // OpenAI互換レスポンスを構築
            json data = json::array();
            int total_tokens = 0;
            for (size_t i = 0; i < embeddings.size(); ++i) {
                data.push_back({
                    {"object", "embedding"},
                    {"embedding", embeddings[i]},
                    {"index", static_cast<int>(i)}
                });
                // トークン数の概算（文字数 / 4）
                total_tokens += static_cast<int>(inputs[i].size() / 4 + 1);
            }

            json resp = {
                {"object", "list"},
                {"data", data},
                {"model", model},
                {"usage", {{"prompt_tokens", total_tokens}, {"total_tokens", total_tokens}}}
            };
            setJson(res, resp);
        } catch (const std::exception& e) {
            respondError(res, 500, "internal_error", std::string("embedding error: ") + e.what());
        } catch (...) {
            respondError(res, 400, "bad_request", "invalid JSON body");
        }
    });
}

void OpenAIEndpoints::setJson(httplib::Response& res, const nlohmann::json& body) {
    res.set_content(body.dump(), "application/json");
}

void OpenAIEndpoints::respondError(httplib::Response& res, int status, const std::string& code, const std::string& message) {
    res.status = status;
    std::string type = "invalid_request_error";
    if (status >= 500) {
        type = "internal_error";
    } else if (status == 429) {
        type = "rate_limit_error";
    } else if (status == 503) {
        type = "service_unavailable";
    }
    setJson(res, {{"error", {{"message", message}, {"type", type}, {"code", code}}}});
}

bool OpenAIEndpoints::validateModel(const std::string& model,
                                    const std::string& capability,
                                    httplib::Response& res) {
    if (model.empty()) {
        respondError(res, 400, "model_required", "model is required");
        return false;
    }
    // Check local registry first
    const auto parsed = parse_model_name_with_quantization(model);
    if (!parsed.valid || parsed.name.empty()) {
        respondError(res, 400, "invalid_request", "model is invalid");
        return false;
    }
    const bool in_registry = registry_.hasModel(parsed.name);
    if (in_registry && !engine_.isInitialized()) {
        return true;
    }
    // Try to resolve/load via ModelResolver (local -> shared -> router API)
    // loadModel() handles the full resolution flow
    auto load_result = engine_.loadModel(model, capability);
    if (!load_result.success) {
        const std::string prefix = "Model does not support capability:";
        if (load_result.error_code == xllm::EngineErrorCode::kOomVram ||
            load_result.error_code == xllm::EngineErrorCode::kOomRam) {
            respondError(res, 503, "resource_exhausted", load_result.error_message);
            return false;
        }
        if (load_result.error_message.rfind(prefix, 0) == 0) {
            respondError(res, 400, "invalid_request", load_result.error_message);
            return false;
        }
        respondError(res, 404, "model_not_found",
            load_result.error_message.empty() ? "model not found" : load_result.error_message);
        return false;
    }
    return true;
}

}  // namespace xllm
