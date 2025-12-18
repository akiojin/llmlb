#include "core/inference_engine.h"

#include "core/llm_tokenizer.h"
#include "core/onnx_llm_manager.h"
#include "models/model_storage.h"
#include "models/model_sync.h"

#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>

#include <filesystem>
#include <fstream>
#include <cctype>
#include <sstream>
#include <unordered_map>
#include <unordered_set>

namespace llm_node {

namespace {

extern "C" {
void llm_tokenizer_free_string(char* s);
bool llm_chat_template_render(
    const char* template_str,
    const char* messages_json,
    const char* special_tokens_json,
    bool add_generation_prompt,
    char** out_text,
    char** out_error);
}  // extern "C"

std::string stripControlTokens(std::string text) {
    const std::vector<std::string> tokens = {
        "<|start|>", "<|end|>", "<|message|>", "<|channel|>",
        "<|im_start|>", "<|im_end|>", "<s>", "</s>", "<|endoftext|>", "<|eot_id|>",
    };
    for (const auto& t : tokens) {
        size_t pos = 0;
        while ((pos = text.find(t, pos)) != std::string::npos) {
            text.erase(pos, t.size());
        }
    }
    const auto l = text.find_first_not_of(" \t\n\r");
    if (l == std::string::npos) return "";
    const auto r = text.find_last_not_of(" \t\n\r");
    return text.substr(l, r - l + 1);
}

std::string extractGptOssFinalMessage(const std::string& output) {
    const std::string marker = "<|channel|>final<|message|>";
    const std::string end = "<|end|>";

    const size_t mpos = output.rfind(marker);
    if (mpos == std::string::npos) return stripControlTokens(output);
    const size_t start = mpos + marker.size();
    const size_t endpos = output.find(end, start);
    const std::string seg = endpos == std::string::npos ? output.substr(start) : output.substr(start, endpos - start);
    return stripControlTokens(seg);
}

std::string readFileToString(const std::filesystem::path& path) {
    std::ifstream ifs(path, std::ios::binary);
    if (!ifs.is_open()) return "";
    std::ostringstream oss;
    oss << ifs.rdbuf();
    return oss.str();
}

std::optional<std::string> loadChatTemplateFromModelDir(const std::filesystem::path& model_dir) {
    const auto jinja = model_dir / "chat_template.jinja";
    if (std::filesystem::exists(jinja)) {
        auto s = readFileToString(jinja);
        if (!s.empty()) return s;
    }

    const auto meta = model_dir / "metadata.json";
    if (std::filesystem::exists(meta)) {
        try {
            std::ifstream ifs(meta);
            auto j = nlohmann::json::parse(ifs);
            if (j.contains("chat_template") && j["chat_template"].is_string()) {
                const auto s = j["chat_template"].get<std::string>();
                if (!s.empty()) return s;
            }
        } catch (...) {
            // ignore
        }
    }

    return std::nullopt;
}

bool isHarmonyChatTemplate(const std::string& tmpl) {
    return tmpl.find("<|channel|>final<|message|>") != std::string::npos &&
           tmpl.find("<|start|>system<|message|>") != std::string::npos &&
           tmpl.find("<|start|>assistant") != std::string::npos;
}

std::optional<nlohmann::json> readJsonFile(const std::filesystem::path& path) {
    if (!std::filesystem::exists(path)) return std::nullopt;
    try {
        std::ifstream ifs(path);
        auto j = nlohmann::json::parse(ifs);
        if (!j.is_object()) return std::nullopt;
        return j;
    } catch (...) {
        return std::nullopt;
    }
}

void mergeTokenString(nlohmann::json& out, const nlohmann::json& src, const std::string& key) {
    if (out.contains(key)) return;
    if (!src.contains(key)) return;
    const auto& v = src.at(key);
    if (v.is_string()) {
        out[key] = v.get<std::string>();
        return;
    }
    if (v.is_object() && v.contains("content") && v.at("content").is_string()) {
        out[key] = v.at("content").get<std::string>();
        return;
    }
}

nlohmann::json loadSpecialTokensFromModelDir(const std::filesystem::path& model_dir) {
    nlohmann::json out = nlohmann::json::object();

    if (auto stm = readJsonFile(model_dir / "special_tokens_map.json")) {
        for (const auto& k : std::vector<std::string>{
                 "bos_token",
                 "eos_token",
                 "unk_token",
                 "pad_token",
                 "sep_token",
                 "cls_token",
                 "mask_token",
             }) {
            mergeTokenString(out, *stm, k);
        }
        if (!out.contains("additional_special_tokens") &&
            stm->contains("additional_special_tokens") &&
            (*stm)["additional_special_tokens"].is_array()) {
            out["additional_special_tokens"] = (*stm)["additional_special_tokens"];
        }
    }

    if (auto tc = readJsonFile(model_dir / "tokenizer_config.json")) {
        for (const auto& k : std::vector<std::string>{
                 "bos_token",
                 "eos_token",
                 "unk_token",
                 "pad_token",
                 "sep_token",
                 "cls_token",
                 "mask_token",
             }) {
            mergeTokenString(out, *tc, k);
        }
        if (!out.contains("additional_special_tokens") &&
            tc->contains("additional_special_tokens") &&
            (*tc)["additional_special_tokens"].is_array()) {
            out["additional_special_tokens"] = (*tc)["additional_special_tokens"];
        }
    }

    return out;
}

std::string renderChatTemplate(
    const std::string& tmpl,
    const std::vector<ChatMessage>& messages,
    const nlohmann::json& special_tokens,
    bool add_generation_prompt) {
    nlohmann::json msgs = nlohmann::json::array();
    msgs.get_ref<nlohmann::json::array_t&>().reserve(messages.size());
    for (const auto& m : messages) {
        msgs.push_back({{"role", m.role}, {"content", m.content}});
    }

    char* out_text = nullptr;
    char* out_error = nullptr;
    const std::string msgs_json = msgs.dump();
    const std::string specials_json = special_tokens.dump();
    if (!llm_chat_template_render(
            tmpl.c_str(),
            msgs_json.c_str(),
            specials_json.c_str(),
            add_generation_prompt,
            &out_text,
            &out_error)) {
        std::string msg = out_error ? std::string(out_error) : std::string("render failed");
        if (out_error) llm_tokenizer_free_string(out_error);
        throw std::runtime_error("chat_template render failed: " + msg);
    }

    std::string rendered = out_text ? std::string(out_text) : std::string();
    if (out_text) llm_tokenizer_free_string(out_text);
    if (out_error) llm_tokenizer_free_string(out_error);
    return rendered;
}

#ifdef USE_ONNX_RUNTIME

struct KvNames {
    int index{0};
    std::string key_in;
    std::string value_in;
    std::string key_out;
    std::string value_out;
};

struct OnnxTextGenIo {
    std::string input_ids;
    std::string attention_mask;
    std::string position_ids;
    std::string logits;
    std::vector<KvNames> kv;

    // For creating empty past tensors.
    int64_t num_heads{0};
    int64_t head_dim{0};

    ONNXTensorElementDataType past_type{ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT};
    ONNXTensorElementDataType logits_type{ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT};
};

std::optional<std::pair<int, std::string>> parseIndexedSuffix(const std::string& name, const std::string& prefix) {
    if (name.rfind(prefix, 0) != 0) return std::nullopt;
    const size_t start = prefix.size();
    const size_t dot = name.find('.', start);
    if (dot == std::string::npos) return std::nullopt;
    const std::string idx_str = name.substr(start, dot - start);
    int idx = 0;
    try {
        idx = std::stoi(idx_str);
    } catch (...) {
        return std::nullopt;
    }
    return std::make_pair(idx, name.substr(dot + 1));
}

OnnxTextGenIo inspectTextGenWithPast(const Ort::Session& session) {
    OnnxTextGenIo io;
    Ort::AllocatorWithDefaultOptions allocator;

    // Inputs
    const size_t input_count = session.GetInputCount();
    std::unordered_map<int, KvNames> kv_by_index;
    for (size_t i = 0; i < input_count; ++i) {
        auto name_alloc = session.GetInputNameAllocated(i, allocator);
        const std::string name = name_alloc.get();

        if (name == "input_ids") io.input_ids = name;
        else if (name == "attention_mask") io.attention_mask = name;
        else if (name == "position_ids") io.position_ids = name;

        if (auto parsed = parseIndexedSuffix(name, "past_key_values.")) {
            const int idx = parsed->first;
            const std::string tail = parsed->second;
            auto& kv = kv_by_index[idx];
            kv.index = idx;
            if (tail == "key") kv.key_in = name;
            if (tail == "value") kv.value_in = name;

            // Capture type/shape from the first past input we see.
            if (io.num_heads == 0 || io.head_dim == 0) {
                // NOTE: Keep Ort::TypeInfo alive while reading TensorTypeAndShapeInfo.
                // Some builds expose TensorTypeAndShapeInfo as a view into TypeInfo.
                auto ort_type_info = session.GetInputTypeInfo(i);
                auto tensor_info = ort_type_info.GetTensorTypeAndShapeInfo();
                io.past_type = static_cast<ONNXTensorElementDataType>(tensor_info.GetElementType());
                auto shape = tensor_info.GetShape();
                // Expected: [batch, n_heads, past_len, head_dim]
                if (shape.size() >= 4) {
                    if (shape[1] > 0) io.num_heads = shape[1];
                    if (shape[3] > 0) io.head_dim = shape[3];
                }
            }
        }
    }

    // Outputs
    const size_t output_count = session.GetOutputCount();
    for (size_t i = 0; i < output_count; ++i) {
        auto name_alloc = session.GetOutputNameAllocated(i, allocator);
        const std::string name = name_alloc.get();

        if (name == "logits") {
            io.logits = name;
            auto ort_type_info = session.GetOutputTypeInfo(i);
            auto tensor_info = ort_type_info.GetTensorTypeAndShapeInfo();
            io.logits_type = static_cast<ONNXTensorElementDataType>(tensor_info.GetElementType());
        }

        if (auto parsed = parseIndexedSuffix(name, "present.")) {
            const int idx = parsed->first;
            const std::string tail = parsed->second;
            auto& kv = kv_by_index[idx];
            kv.index = idx;
            if (tail == "key") kv.key_out = name;
            if (tail == "value") kv.value_out = name;
        }
    }

    // Finalize kv list in index order.
    io.kv.reserve(kv_by_index.size());
    for (auto& [idx, kv] : kv_by_index) {
        if (kv.key_in.empty() || kv.value_in.empty() || kv.key_out.empty() || kv.value_out.empty()) {
            continue;
        }
        io.kv.push_back(std::move(kv));
    }
    std::sort(io.kv.begin(), io.kv.end(), [](const KvNames& a, const KvNames& b) { return a.index < b.index; });

    return io;
}

Ort::Value createI64Tensor(const std::vector<int64_t>& data, const std::vector<int64_t>& shape) {
    // Use the standard CPU arena allocator. Some EPs / bindings are sensitive to mem types.
    Ort::MemoryInfo mem = Ort::MemoryInfo::CreateCpu(OrtArenaAllocator, OrtMemTypeDefault);
    return Ort::Value::CreateTensor<int64_t>(
        mem,
        const_cast<int64_t*>(data.data()),
        data.size(),
        shape.data(),
        shape.size());
}

Ort::Value createFloatTensor(ONNXTensorElementDataType type, std::vector<uint8_t>& backing, const std::vector<int64_t>& shape) {
    Ort::MemoryInfo mem = Ort::MemoryInfo::CreateCpu(OrtArenaAllocator, OrtMemTypeDefault);
    const size_t bytes = backing.size();
    return Ort::Value::CreateTensor(
        mem,
        backing.data(),
        bytes,
        shape.data(),
        shape.size(),
        type);
}

Ort::Value createAllocatedTensor(ONNXTensorElementDataType type, const std::vector<int64_t>& shape) {
    Ort::AllocatorWithDefaultOptions allocator;
    return Ort::Value::CreateTensor(
        allocator,
        shape.data(),
        shape.size(),
        type);
}

float f16ToF32(uint16_t h) {
    const uint32_t sign = (h & 0x8000u) << 16;
    uint32_t exp = (h & 0x7C00u) >> 10;
    uint32_t mant = (h & 0x03FFu);

    uint32_t f;
    if (exp == 0) {
        if (mant == 0) {
            f = sign;
        } else {
            // subnormal
            exp = 1;
            while ((mant & 0x0400u) == 0) {
                mant <<= 1;
                exp--;
            }
            mant &= 0x03FFu;
            const uint32_t exp_f = (exp + (127 - 15)) << 23;
            const uint32_t mant_f = mant << 13;
            f = sign | exp_f | mant_f;
        }
    } else if (exp == 0x1Fu) {
        // inf/nan
        f = sign | 0x7F800000u | (mant << 13);
    } else {
        const uint32_t exp_f = (exp + (127 - 15)) << 23;
        const uint32_t mant_f = mant << 13;
        f = sign | exp_f | mant_f;
    }
    float out;
    std::memcpy(&out, &f, sizeof(out));
    return out;
}

int64_t argmaxFromLogits(const Ort::Value& logits, ONNXTensorElementDataType type) {
    const auto info = logits.GetTensorTypeAndShapeInfo();
    const auto shape = info.GetShape();
    if (shape.size() < 3) {
        throw std::runtime_error("Invalid logits shape");
    }
    const int64_t seq_len = shape[1];
    const int64_t vocab = shape[2];
    if (seq_len <= 0 || vocab <= 0) {
        throw std::runtime_error("Invalid logits dims");
    }

    const size_t vocab_sz = static_cast<size_t>(vocab);
    const size_t last_offset = static_cast<size_t>(seq_len - 1) * vocab_sz;

    int64_t best_id = 0;
    float best_val = -1e30f;

    if (type == ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT) {
        const float* data = logits.GetTensorData<float>();
        for (size_t i = 0; i < vocab_sz; ++i) {
            const float v = data[last_offset + i];
            if (v > best_val) {
                best_val = v;
                best_id = static_cast<int64_t>(i);
            }
        }
        return best_id;
    }

    if (type == ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT16) {
        const uint16_t* data = reinterpret_cast<const uint16_t*>(logits.GetTensorRawData());
        for (size_t i = 0; i < vocab_sz; ++i) {
            const float v = f16ToF32(data[last_offset + i]);
            if (v > best_val) {
                best_val = v;
                best_id = static_cast<int64_t>(i);
            }
        }
        return best_id;
    }

    throw std::runtime_error("Unsupported logits dtype (expected float/float16)");
}

Ort::Value findOutputByName(std::vector<Ort::Value>& outputs, const std::vector<std::string>& names, const std::string& target) {
    for (size_t i = 0; i < names.size() && i < outputs.size(); ++i) {
        if (names[i] == target) {
            return std::move(outputs[i]);
        }
    }
    throw std::runtime_error("Missing required output: " + target);
}

#endif  // USE_ONNX_RUNTIME

}  // namespace

// テスト用に公開する薄いラッパー（本番コードには影響なし）
std::string extractGptOssFinalMessageForTest(const std::string& output) {
    return extractGptOssFinalMessage(output);
}

InferenceEngine::InferenceEngine(OnnxLlmManager& manager, ModelStorage& model_storage, ModelSync* model_sync)
    : manager_(&manager)
    , model_storage_(&model_storage)
    , model_sync_(model_sync) {}

std::string InferenceEngine::buildChatPrompt(const std::vector<ChatMessage>& messages) const {
    std::ostringstream oss;
    for (const auto& msg : messages) {
        if (msg.role == "system") {
            oss << "System: " << msg.content << "\n\n";
        } else if (msg.role == "user") {
            oss << "User: " << msg.content << "\n\n";
        } else if (msg.role == "assistant") {
            oss << "Assistant: " << msg.content << "\n\n";
        }
    }
    oss << "Assistant: ";
    return oss.str();
}

std::string InferenceEngine::generateChat(
    const std::vector<ChatMessage>& messages,
    const std::string& model_name,
    const InferenceParams& params) const {

    if (messages.empty()) return "";

    // Stub mode (tests) when dependencies are not injected.
    if (!isInitialized()) {
        spdlog::warn("InferenceEngine not initialized, using stub mode");
        return "Response to: " + messages.back().content;
    }

    auto lr = const_cast<InferenceEngine*>(this)->loadModel(model_name);
    if (!lr.success) throw std::runtime_error(lr.error_message.empty() ? "Failed to load model" : lr.error_message);
    if (lr.model_path.empty()) throw std::runtime_error("Failed to resolve model path");

#ifndef USE_ONNX_RUNTIME
    (void)params;
    throw std::runtime_error("ONNX Runtime not available");
#else
    Ort::Session* session = manager_->getSession(lr.model_path);
    if (session == nullptr) {
        throw std::runtime_error("Model session not loaded: " + lr.model_path);
    }

    const std::filesystem::path model_dir = std::filesystem::path(lr.model_path).parent_path();
    const std::string tokenizer_path = (model_dir / "tokenizer.json").string();
    auto tokenizer = LlmTokenizer::loadFromTokenizerJson(tokenizer_path);

    const auto tmpl = loadChatTemplateFromModelDir(model_dir);
    const bool harmony = tmpl.has_value() && isHarmonyChatTemplate(*tmpl);
    const auto special_tokens = loadSpecialTokensFromModelDir(model_dir);
    std::string prompt;
    if (tmpl.has_value()) {
        try {
            prompt = renderChatTemplate(*tmpl, messages, special_tokens, true);
        } catch (const std::exception& e) {
            spdlog::warn("chat_template render failed, falling back to default prompt: {}", e.what());
            prompt = buildChatPrompt(messages);
        }
    } else {
        prompt = buildChatPrompt(messages);
    }

    const auto prompt_ids = tokenizer->encode(prompt, false);
    if (prompt_ids.empty()) return "";

    const auto io = inspectTextGenWithPast(*session);
    if (io.input_ids.empty() || io.attention_mask.empty() || io.position_ids.empty() || io.logits.empty() || io.kv.empty()) {
        throw std::runtime_error(
            "Unsupported ONNX text generation model (expected text-generation-with-past). "
            "Re-export with optimum --task text-generation-with-past.");
    }
    if (io.num_heads <= 0 || io.head_dim <= 0) {
        throw std::runtime_error("Could not infer KV cache shape from ONNX model inputs");
    }

    // Resolve stop token ids (best-effort; different models use different stop tokens).
    std::unordered_set<int64_t> stop_ids;
    auto add_stop = [&](const std::string& tok) {
        if (tok.empty()) return;
        if (auto id = tokenizer->tokenToId(tok)) stop_ids.insert(*id);
    };
    if (special_tokens.contains("eos_token") && special_tokens["eos_token"].is_string()) {
        add_stop(special_tokens["eos_token"].get<std::string>());
    }
    if (special_tokens.contains("additional_special_tokens") && special_tokens["additional_special_tokens"].is_array()) {
        for (const auto& t : special_tokens["additional_special_tokens"]) {
            if (t.is_string()) add_stop(t.get<std::string>());
        }
    }
    for (const auto& s : std::vector<std::string>{"<|end|>", "<|return|>", "<|endoftext|>", "</s>", "<|eot_id|>"}) {
        add_stop(s);
    }

    std::vector<int64_t> generated_ids;
    generated_ids.reserve(params.max_tokens);

    // Cached KV tensors (use ORT outputs as next inputs to avoid copies).
    std::vector<Ort::Value> past_in;        // 2*layers
    std::vector<int64_t> past_shape = {1, io.num_heads, 0, io.head_dim};
    const size_t layers = io.kv.size();
    past_in.reserve(layers * 2);

    const size_t elem_size =
        (io.past_type == ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT16) ? 2u :
        (io.past_type == ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT) ? 4u : 0u;
    if (elem_size == 0) {
        throw std::runtime_error("Unsupported KV dtype (expected float/float16)");
    }

    // Empty past tensors for prefill (past_len = 0).
    for (size_t i = 0; i < io.kv.size(); ++i) {
        past_in.push_back(createAllocatedTensor(io.past_type, past_shape));
        past_in.push_back(createAllocatedTensor(io.past_type, past_shape));
    }

    int64_t past_len = 0;

    auto run_step = [&](const std::vector<int64_t>& step_ids) -> std::vector<Ort::Value> {
        const int64_t seq_len = static_cast<int64_t>(step_ids.size());

        // Build input tensors
        std::vector<int64_t> input_ids = step_ids;
        const std::vector<int64_t> input_shape = {1, seq_len};

        std::vector<int64_t> attn(past_len + seq_len, 1);
        const std::vector<int64_t> attn_shape = {1, static_cast<int64_t>(attn.size())};

        std::vector<int64_t> pos;
        pos.reserve(seq_len);
        for (int64_t i = 0; i < seq_len; ++i) pos.push_back(past_len + i);
        const std::vector<int64_t> pos_shape = {1, seq_len};

        std::vector<std::string> input_names;
        input_names.reserve(3 + layers * 2);
        input_names.push_back(io.input_ids);
        input_names.push_back(io.attention_mask);
        input_names.push_back(io.position_ids);
        for (const auto& kv : io.kv) {
            input_names.push_back(kv.key_in);
            input_names.push_back(kv.value_in);
        }

        std::vector<const char*> input_name_ptrs;
        input_name_ptrs.reserve(input_names.size());
        for (const auto& s : input_names) input_name_ptrs.push_back(s.c_str());

        std::vector<Ort::Value> input_tensors;
        input_tensors.reserve(input_names.size());
        input_tensors.push_back(createI64Tensor(input_ids, input_shape));
        input_tensors.push_back(createI64Tensor(attn, attn_shape));
        input_tensors.push_back(createI64Tensor(pos, pos_shape));
        for (auto& v : past_in) {
            input_tensors.push_back(std::move(v));
        }

        std::vector<std::string> output_names;
        output_names.reserve(1 + layers * 2);
        output_names.push_back(io.logits);
        for (const auto& kv : io.kv) {
            output_names.push_back(kv.key_out);
            output_names.push_back(kv.value_out);
        }
        std::vector<const char*> output_name_ptrs;
        output_name_ptrs.reserve(output_names.size());
        for (const auto& s : output_names) output_name_ptrs.push_back(s.c_str());

        // Force outputs to CPU memory so we can read logits safely for sampling.
        Ort::IoBinding binding(*session);
        for (size_t i = 0; i < input_name_ptrs.size() && i < input_tensors.size(); ++i) {
            binding.BindInput(input_name_ptrs[i], input_tensors[i]);
        }
        Ort::MemoryInfo cpu_mem = Ort::MemoryInfo::CreateCpu(OrtArenaAllocator, OrtMemTypeDefault);
        for (size_t i = 0; i < output_name_ptrs.size(); ++i) {
            binding.BindOutput(output_name_ptrs[i], cpu_mem);
        }
        binding.SynchronizeInputs();
        session->Run(Ort::RunOptions{nullptr}, binding);
        binding.SynchronizeOutputs();
        auto outputs = binding.GetOutputValues();

        // Restore past_in from outputs (skip logits at index 0)
        past_in.clear();
        past_in.reserve(layers * 2);
        for (size_t i = 1; i < outputs.size(); ++i) {
            past_in.push_back(std::move(outputs[i]));
        }

        past_len += seq_len;
        return outputs;
    };

    // Prefill: feed the full prompt.
    auto outputs = run_step(prompt_ids);
    Ort::Value logits = std::move(outputs[0]);

    for (size_t step = 0; step < params.max_tokens; ++step) {
        const int64_t next_id = argmaxFromLogits(logits, io.logits_type);
        if (!stop_ids.empty() && stop_ids.count(next_id)) {
            break;
        }
        generated_ids.push_back(next_id);

        // Decode step: feed just the last token.
        outputs = run_step({next_id});
        logits = std::move(outputs[0]);
    }

    if (generated_ids.empty()) return "";
    const std::string raw = tokenizer->decode(generated_ids, false);
    return harmony ? extractGptOssFinalMessage(raw) : stripControlTokens(raw);
#endif
}

std::string InferenceEngine::generateCompletion(
    const std::string& prompt,
    const std::string& model,
    const InferenceParams& params) const {
    std::vector<ChatMessage> msgs;
    msgs.push_back({"user", prompt});
    return generateChat(msgs, model, params);
}

std::vector<std::string> InferenceEngine::generateChatStream(
    const std::vector<ChatMessage>& messages,
    const std::string& model,
    const InferenceParams& params,
    const std::function<void(const std::string&)>& on_token) const {
    if (messages.empty()) return {};

    // Stub mode (tests) when dependencies are not injected.
    if (!isInitialized()) {
        const std::string text = generateChat(messages, model, params);
        auto tokens = generateTokens(text, params.max_tokens);
        for (const auto& t : tokens) {
            if (on_token) on_token(t);
        }
        return tokens;
    }

#ifndef USE_ONNX_RUNTIME
    throw std::runtime_error("ONNX Runtime not available");
#else
    auto lr = const_cast<InferenceEngine*>(this)->loadModel(model);
    if (!lr.success) throw std::runtime_error(lr.error_message.empty() ? "Failed to load model" : lr.error_message);
    if (lr.model_path.empty()) throw std::runtime_error("Failed to resolve model path");

    Ort::Session* session = manager_->getSession(lr.model_path);
    if (session == nullptr) {
        throw std::runtime_error("Model session not loaded: " + lr.model_path);
    }

    const std::filesystem::path model_dir = std::filesystem::path(lr.model_path).parent_path();
    const std::string tokenizer_path = (model_dir / "tokenizer.json").string();
    auto tokenizer = LlmTokenizer::loadFromTokenizerJson(tokenizer_path);

    const auto tmpl = loadChatTemplateFromModelDir(model_dir);
    const bool harmony = tmpl.has_value() && isHarmonyChatTemplate(*tmpl);
    const auto special_tokens = loadSpecialTokensFromModelDir(model_dir);
    std::string prompt;
    if (tmpl.has_value()) {
        try {
            prompt = renderChatTemplate(*tmpl, messages, special_tokens, true);
        } catch (const std::exception& e) {
            spdlog::warn("chat_template render failed, falling back to default prompt: {}", e.what());
            prompt = buildChatPrompt(messages);
        }
    } else {
        prompt = buildChatPrompt(messages);
    }

    const auto prompt_ids = tokenizer->encode(prompt, false);
    if (prompt_ids.empty()) return {};

    const auto io = inspectTextGenWithPast(*session);
    if (io.input_ids.empty() || io.attention_mask.empty() || io.position_ids.empty() || io.logits.empty() || io.kv.empty()) {
        throw std::runtime_error(
            "Unsupported ONNX text generation model (expected text-generation-with-past). "
            "Re-export with optimum --task text-generation-with-past.");
    }
    if (io.num_heads <= 0 || io.head_dim <= 0) {
        throw std::runtime_error("Could not infer KV cache shape from ONNX model inputs");
    }

    std::unordered_set<int64_t> stop_ids;
    auto add_stop = [&](const std::string& tok) {
        if (tok.empty()) return;
        if (auto id = tokenizer->tokenToId(tok)) stop_ids.insert(*id);
    };
    if (special_tokens.contains("eos_token") && special_tokens["eos_token"].is_string()) {
        add_stop(special_tokens["eos_token"].get<std::string>());
    }
    if (special_tokens.contains("additional_special_tokens") && special_tokens["additional_special_tokens"].is_array()) {
        for (const auto& t : special_tokens["additional_special_tokens"]) {
            if (t.is_string()) add_stop(t.get<std::string>());
        }
    }
    for (const auto& s : std::vector<std::string>{"<|end|>", "<|return|>", "<|endoftext|>", "</s>", "<|eot_id|>"}) {
        add_stop(s);
    }

    std::vector<std::string> streamed;

    std::vector<Ort::Value> past_in;
    std::vector<int64_t> past_shape = {1, io.num_heads, 0, io.head_dim};
    const size_t layers = io.kv.size();
    past_in.reserve(layers * 2);

    const size_t elem_size =
        (io.past_type == ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT16) ? 2u :
        (io.past_type == ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT) ? 4u : 0u;
    if (elem_size == 0) {
        throw std::runtime_error("Unsupported KV dtype (expected float/float16)");
    }

    for (size_t i = 0; i < io.kv.size(); ++i) {
        past_in.push_back(createAllocatedTensor(io.past_type, past_shape));
        past_in.push_back(createAllocatedTensor(io.past_type, past_shape));
    }

    int64_t past_len = 0;

    auto run_step = [&](const std::vector<int64_t>& step_ids) -> std::vector<Ort::Value> {
        const int64_t seq_len = static_cast<int64_t>(step_ids.size());

        std::vector<int64_t> input_ids = step_ids;
        const std::vector<int64_t> input_shape = {1, seq_len};

        std::vector<int64_t> attn(past_len + seq_len, 1);
        const std::vector<int64_t> attn_shape = {1, static_cast<int64_t>(attn.size())};

        std::vector<int64_t> pos;
        pos.reserve(seq_len);
        for (int64_t i = 0; i < seq_len; ++i) pos.push_back(past_len + i);
        const std::vector<int64_t> pos_shape = {1, seq_len};

        std::vector<std::string> input_names;
        input_names.reserve(3 + layers * 2);
        input_names.push_back(io.input_ids);
        input_names.push_back(io.attention_mask);
        input_names.push_back(io.position_ids);
        for (const auto& kv : io.kv) {
            input_names.push_back(kv.key_in);
            input_names.push_back(kv.value_in);
        }

        std::vector<const char*> input_name_ptrs;
        input_name_ptrs.reserve(input_names.size());
        for (const auto& s : input_names) input_name_ptrs.push_back(s.c_str());

        std::vector<Ort::Value> input_tensors;
        input_tensors.reserve(input_names.size());
        input_tensors.push_back(createI64Tensor(input_ids, input_shape));
        input_tensors.push_back(createI64Tensor(attn, attn_shape));
        input_tensors.push_back(createI64Tensor(pos, pos_shape));
        for (auto& v : past_in) {
            input_tensors.push_back(std::move(v));
        }

        std::vector<std::string> output_names;
        output_names.reserve(1 + layers * 2);
        output_names.push_back(io.logits);
        for (const auto& kv : io.kv) {
            output_names.push_back(kv.key_out);
            output_names.push_back(kv.value_out);
        }
        std::vector<const char*> output_name_ptrs;
        output_name_ptrs.reserve(output_names.size());
        for (const auto& s : output_names) output_name_ptrs.push_back(s.c_str());

        // Force outputs to CPU memory so we can read logits safely for sampling.
        Ort::IoBinding binding(*session);
        for (size_t i = 0; i < input_name_ptrs.size() && i < input_tensors.size(); ++i) {
            binding.BindInput(input_name_ptrs[i], input_tensors[i]);
        }
        Ort::MemoryInfo cpu_mem = Ort::MemoryInfo::CreateCpu(OrtDeviceAllocator, OrtMemTypeCPU);
        for (size_t i = 0; i < output_name_ptrs.size(); ++i) {
            binding.BindOutput(output_name_ptrs[i], cpu_mem);
        }
        binding.SynchronizeInputs();
        session->Run(Ort::RunOptions{nullptr}, binding);
        binding.SynchronizeOutputs();
        auto outputs = binding.GetOutputValues();

        past_in.clear();
        past_in.reserve(layers * 2);
        for (size_t i = 1; i < outputs.size(); ++i) {
            past_in.push_back(std::move(outputs[i]));
        }

        past_len += seq_len;
        return outputs;
    };

    // Prefill.
    auto outputs = run_step(prompt_ids);
    Ort::Value logits = std::move(outputs[0]);

    std::string raw_accum;
    std::string last_final;

    for (size_t step = 0; step < params.max_tokens; ++step) {
        const int64_t next_id = argmaxFromLogits(logits, io.logits_type);
        if (!stop_ids.empty() && stop_ids.count(next_id)) {
            break;
        }

        // Decode this token.
        const std::string piece = tokenizer->decode(std::vector<int64_t>{next_id}, false);
        raw_accum += piece;
        const std::string cur_final = harmony ? extractGptOssFinalMessage(raw_accum) : stripControlTokens(raw_accum);
        std::string delta;
        if (!last_final.empty() && cur_final.rfind(last_final, 0) == 0) {
            delta = cur_final.substr(last_final.size());
        } else if (cur_final != last_final) {
            delta = cur_final;
        }
        if (!delta.empty()) {
            streamed.push_back(delta);
            if (on_token) on_token(delta);
            last_final = cur_final;
        }

        outputs = run_step({next_id});
        logits = std::move(outputs[0]);
    }

    return streamed;
#endif
}

std::vector<std::string> InferenceEngine::generateChatStream(
    const std::vector<ChatMessage>& messages,
    size_t max_tokens,
    const std::function<void(const std::string&)>& on_token) const {
    const std::string text = generateChat(messages, "");
    auto tokens = generateTokens(text, max_tokens);
    for (const auto& t : tokens) {
        if (on_token) on_token(t);
    }
    return tokens;
}

std::vector<std::vector<std::string>> InferenceEngine::generateBatch(
    const std::vector<std::string>& prompts,
    size_t max_tokens) const {
    std::vector<std::vector<std::string>> outputs;
    outputs.reserve(prompts.size());
    for (const auto& p : prompts) {
        outputs.push_back(generateTokens(p, max_tokens));
    }
    return outputs;
}

std::vector<std::string> InferenceEngine::generateTokens(
    const std::string& prompt,
    size_t max_tokens) const {
    std::vector<std::string> tokens;
    std::string current;

    for (char c : prompt) {
        if (std::isspace(static_cast<unsigned char>(c))) {
            if (!current.empty()) {
                tokens.push_back(current);
                if (tokens.size() >= max_tokens) break;
                current.clear();
            }
        } else {
            current.push_back(c);
        }
    }
    if (!current.empty() && tokens.size() < max_tokens) {
        tokens.push_back(current);
    }
    return tokens;
}

std::string InferenceEngine::sampleNextToken(const std::vector<std::string>& tokens) const {
    if (tokens.empty()) return "";
    return tokens.back();
}

ModelLoadResult InferenceEngine::loadModel(const std::string& model_name) {
    ModelLoadResult result;

    if (!isInitialized()) {
        result.error_message = "InferenceEngine not initialized";
        return result;
    }

    // 1) Local storage (SPEC-dcaeaec4) - prefer ONNX (model.onnx), fallback to legacy GGUF.
    std::string model_path = model_storage_->resolveOnnx(model_name);
    if (model_path.empty()) {
        model_path = model_storage_->resolveGguf(model_name);
    }

    // 2) Remote path from router (if configured).
    if (model_path.empty() && model_sync_ != nullptr) {
        model_path = model_sync_->getRemotePath(model_name);
        if (!model_path.empty()) {
            spdlog::info("Using remote path for model {}: {}", model_name, model_path);
        }
    }

    if (model_path.empty()) {
        result.error_message = "Model not found: " + model_name;
        return result;
    }

    if (!manager_->loadModelIfNeeded(model_path)) {
        result.error_message = "Failed to load model: " + model_path;
        return result;
    }

    result.success = true;
    result.model_path = model_path;
    return result;
}

std::vector<std::vector<float>> InferenceEngine::generateEmbeddings(
    const std::vector<std::string>& inputs,
    const std::string& model) const {
    std::vector<std::vector<float>> results;

    // Stub mode: keep existing contract tests stable.
    if (!isInitialized()) {
        spdlog::warn("InferenceEngine not initialized, returning dummy embeddings");
        for (size_t i = 0; i < inputs.size(); ++i) {
            results.push_back({1.0f, 0.0f, -1.0f});
        }
        return results;
    }

    auto lr = const_cast<InferenceEngine*>(this)->loadModel(model);
    if (!lr.success) {
        throw std::runtime_error(lr.error_message.empty() ? "Failed to load model" : lr.error_message);
    }

    // NOTE: Full ONNX embedding inference is not implemented yet.
    for (size_t i = 0; i < inputs.size(); ++i) {
        results.push_back({1.0f, 0.0f, -1.0f});
    }
    return results;
}

}  // namespace llm_node
