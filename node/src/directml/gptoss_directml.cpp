#include <gpt-oss.h>

#include <algorithm>
#include <array>
#include <atomic>
#include <cctype>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <mutex>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#include <nlohmann/json.hpp>

#ifdef _WIN32
#include <windows.h>
#include <d3d12.h>
#include <dxgi1_6.h>
#include <wrl/client.h>
#include <DirectML.h>
#endif

namespace fs = std::filesystem;

namespace {
#ifdef _WIN32
using Microsoft::WRL::ComPtr;
#endif
struct GptossFileHeader {
    char magic[12];
    uint32_t zero;
};

struct GptossUuid {
    uint8_t bytes[16];
};

constexpr std::array<uint8_t, 16> kDirectMlLayoutUuid = {
    0xD8, 0xC0, 0x7A, 0xC6, 0x8F, 0xC8, 0x4A, 0x24, 0xAD, 0x47, 0x34, 0xC8, 0x28, 0xB7, 0x16, 0xA8,
};

struct GptossModelHeader {
    uint32_t context_length;
    uint32_t num_blocks;
    uint32_t num_experts;
    uint32_t num_active_experts;
    uint32_t embedding_dim;
    uint32_t mlp_dim;
    float swiglu_limit;
    uint32_t head_dim;
    uint32_t num_heads;
    uint32_t num_kv_heads;
    uint32_t attention_window;
    float rope_theta;
    float interpolation_scale;
    float yarn_offset;
    float yarn_scale;
    float yarn_multiplier;
    float rmsnorm_epsilon;
};

struct GptossTokenizerHeader {
    uint32_t num_special_tokens;
    uint32_t num_text_tokens;
    uint32_t regex_size;
    uint32_t tokens_size;
};

struct GptossTokenizer {
    std::atomic<uint32_t> ref_count{1};
    std::vector<std::string> id_to_token;
    std::unordered_map<std::string, uint32_t> token_to_id;
    std::array<uint32_t, gptoss_special_token_max - 1> special_ids{};
    std::vector<uint8_t> tokens_blob;
    uint32_t num_special_tokens{0};
    uint32_t num_text_tokens{0};
};

struct DmlPlan {
    size_t kv_cache_elements{0};
    size_t kv_cache_bytes{0};
    size_t vocab_embeddings_elements{0};
    size_t vocab_embeddings_bytes{0};
};

struct DmlTensorLayout {
    uint32_t vocab_size{0};
    uint32_t embedding_dim{0};
    uint32_t context_length{0};
    uint32_t num_blocks{0};
    uint32_t num_heads{0};
    uint32_t num_kv_heads{0};
    uint32_t head_dim{0};
};

struct DmlTensorSpec {
    std::vector<uint32_t> dims;
    uint32_t element_size{0};
};

struct DmlGraph {
#ifdef _WIN32
    struct BufferTensor {
        std::vector<uint32_t> sizes;
        std::vector<uint32_t> strides;
        DML_BUFFER_TENSOR_DESC buffer_desc{};
        DML_TENSOR_DESC tensor_desc{};
    };

    struct OperatorDesc {
        DML_ELEMENT_WISE_IDENTITY_OPERATOR_DESC identity_desc{};
        DML_OPERATOR_DESC op_desc{};
        bool prepared{false};
    };

    struct GraphDesc {
        BufferTensor token_ids;
        BufferTensor logits;
        BufferTensor kv_cache;
        bool initialized{false};
    };

    ComPtr<IDMLCompiledOperator> prefill_op;
    ComPtr<IDMLCompiledOperator> decode_op;
    OperatorDesc prefill_desc;
    OperatorDesc decode_desc;
    GraphDesc desc;
#endif
    DmlTensorLayout layout{};
    std::vector<DmlTensorSpec> prefill_inputs;
    std::vector<DmlTensorSpec> prefill_outputs;
    std::vector<DmlTensorSpec> decode_inputs;
    std::vector<DmlTensorSpec> decode_outputs;
    bool stub_graph{false};
    bool has_prefill{false};
    bool has_decode{false};
    bool initialized{false};
};

struct DmlContextPlan {
    size_t token_buffer_bytes{0};
    size_t logits_buffer_bytes{0};
    size_t kv_cache_bytes{0};
};

struct DmlBuffers {
#ifdef _WIN32
    ComPtr<ID3D12Resource> token_buffer;
    ComPtr<ID3D12Resource> logits_buffer;
    ComPtr<ID3D12Resource> kv_cache_buffer;
    ComPtr<ID3D12Resource> token_upload_buffer;
    ComPtr<ID3D12Resource> logits_readback_buffer;
#endif
    bool initialized{false};
};

struct DmlBindings {
#ifdef _WIN32
    DML_BUFFER_BINDING token_binding{};
    DML_BUFFER_BINDING logits_binding{};
    DML_BUFFER_BINDING kv_cache_binding{};
    DML_BINDING_DESC token_binding_desc{};
    DML_BINDING_DESC logits_binding_desc{};
    DML_BINDING_DESC kv_cache_binding_desc{};
    std::array<DML_BINDING_DESC, 2> decode_input_descs{};
    std::array<DML_BINDING_DESC, 2> prefill_output_descs{};
#endif
    bool initialized{false};
};

struct DmlExecState {
#ifdef _WIN32
    ComPtr<IDMLDevice> dml_device;
    ComPtr<ID3D12CommandAllocator> allocator;
    ComPtr<ID3D12GraphicsCommandList> command_list;
    ComPtr<ID3D12CommandQueue> command_queue;
    ComPtr<ID3D12Fence> fence;
    HANDLE fence_event{nullptr};
    uint64_t fence_value{0};
    ComPtr<IDMLCommandRecorder> recorder;
    ComPtr<IDMLBindingTable> binding_table;
    bool binding_table_for_decode{false};
#endif
    bool initialized{false};
};

struct GptossContext;
uint32_t select_stub_token(const GptossContext& ctx, uint32_t vocab_size);

struct GptossModel {
    std::atomic<uint32_t> ref_count{1};
    std::string model_dir;
    std::string model_path;
    gptoss_tokenizer_t tokenizer{nullptr};
    uint32_t max_context_length{0};
    uint32_t vocabulary_size{0};
    GptossModelHeader header{};
    GptossUuid layout_uuid{};
    size_t weights_offset{0};
    size_t weights_bytes{0};
    bool has_weights_blob{false};
    DmlPlan dml_plan{};
    DmlTensorLayout dml_layout{};
    DmlGraph dml_graph{};
#ifdef _WIN32
    ComPtr<ID3D12Resource> weights_buffer;
#endif
};

bool read_exact(std::ifstream& in, void* out, size_t size) {
    in.read(reinterpret_cast<char*>(out), static_cast<std::streamsize>(size));
    return in.good() && in.gcount() == static_cast<std::streamsize>(size);
}

bool uuid_equals(const GptossUuid& uuid, const std::array<uint8_t, 16>& value) {
    return std::memcmp(uuid.bytes, value.data(), value.size()) == 0;
}

gptoss_special_token decode_special_token_uuid(const GptossUuid& uuid) {
    static const std::array<std::pair<std::array<uint8_t, 16>, gptoss_special_token>, 9> mapping = {{
        {{{0x55, 0xA7, 0x7C, 0x2F, 0x8A, 0x01, 0x4C, 0x54, 0x8A, 0xC2, 0x31, 0x3B, 0xFC, 0x7E, 0x20, 0x8D}}, gptoss_special_token_start},
        {{{0x16, 0xE4, 0x04, 0x31, 0xF4, 0x7F, 0x4B, 0x22, 0xB5, 0x9B, 0x8B, 0x27, 0x8F, 0xC3, 0x0A, 0x54}}, gptoss_special_token_message},
        {{{0xFC, 0xAC, 0x2F, 0x6D, 0x47, 0x05, 0x4F, 0x6B, 0xB2, 0x28, 0x64, 0x2A, 0xCC, 0xAC, 0x72, 0x38}}, gptoss_special_token_end},
        {{{0xF7, 0x99, 0xFF, 0x69, 0x19, 0x92, 0x43, 0xC4, 0xA3, 0xD8, 0xD8, 0x31, 0xF4, 0x75, 0xDC, 0x75}}, gptoss_special_token_return},
        {{{0xE1, 0x5B, 0xA7, 0x02, 0x28, 0xC4, 0x42, 0x92, 0xAB, 0x8F, 0xFF, 0xA4, 0x34, 0x70, 0x91, 0x28}}, gptoss_special_token_refusal},
        {{{0xC0, 0xBB, 0x14, 0xC7, 0x60, 0x22, 0x49, 0xDA, 0xAD, 0x08, 0x79, 0x2D, 0x67, 0xE8, 0xB4, 0x70}}, gptoss_special_token_constrain},
        {{{0xFD, 0x3D, 0xDA, 0x11, 0xC8, 0xAB, 0x40, 0x33, 0x87, 0x6E, 0xD9, 0x3D, 0xEB, 0x17, 0x2C, 0x93}}, gptoss_special_token_channel},
        {{{0x12, 0x20, 0xF7, 0x96, 0xE3, 0x88, 0x4D, 0xE5, 0xB4, 0x87, 0xFE, 0x2E, 0xB5, 0xFE, 0x03, 0xC0}}, gptoss_special_token_call},
        {{{0x07, 0xD7, 0xDA, 0x55, 0xB3, 0x46, 0x4C, 0xFF, 0x8B, 0x37, 0x7C, 0xEF, 0xAC, 0xF8, 0xA3, 0xE8}}, gptoss_special_token_untrusted},
    }};
    for (const auto& entry : mapping) {
        if (uuid_equals(uuid, entry.first)) {
            return entry.second;
        }
    }
    static const std::array<uint8_t, 16> kEndUntrusted = {
        0xF2, 0x65, 0xBD, 0x9C, 0xC7, 0x17, 0x46, 0x9E, 0xA4, 0x47, 0x92, 0x06, 0x87, 0xD6, 0x5D, 0x90,
    };
    if (uuid_equals(uuid, kEndUntrusted)) {
        return gptoss_special_token_end_untrusted;
    }
    return gptoss_special_token_invalid;
}

bool build_dml_plan(const GptossModelHeader& header, uint32_t vocabulary_size, DmlPlan& plan) {
    if (header.context_length == 0 || header.num_blocks == 0 || header.num_kv_heads == 0 || header.head_dim == 0) {
        return false;
    }
    const size_t kv_elements =
        static_cast<size_t>(header.num_blocks) *
        static_cast<size_t>(header.context_length) *
        static_cast<size_t>(header.num_kv_heads) *
        static_cast<size_t>(header.head_dim) * 2;
    plan.kv_cache_elements = kv_elements;
    plan.kv_cache_bytes = kv_elements * sizeof(float);

    if (vocabulary_size != 0 && header.embedding_dim != 0) {
        const size_t embed_elements =
            static_cast<size_t>(vocabulary_size) * static_cast<size_t>(header.embedding_dim);
        plan.vocab_embeddings_elements = embed_elements;
        plan.vocab_embeddings_bytes = embed_elements * sizeof(float);
    }
    return true;
}

bool build_dml_tensor_layout(const GptossModelHeader& header,
                             uint32_t vocabulary_size,
                             DmlTensorLayout& layout) {
    if (vocabulary_size == 0 || header.embedding_dim == 0) return false;
    if (header.num_heads == 0 || header.num_kv_heads == 0 || header.head_dim == 0) return false;
    layout.vocab_size = vocabulary_size;
    layout.embedding_dim = header.embedding_dim;
    layout.context_length = header.context_length;
    layout.num_blocks = header.num_blocks;
    layout.num_heads = header.num_heads;
    layout.num_kv_heads = header.num_kv_heads;
    layout.head_dim = header.head_dim;
    return true;
}

bool validate_tensor_spec(const DmlTensorSpec& spec) {
    if (spec.element_size == 0) return false;
    if (spec.dims.empty()) return false;
    for (auto dim : spec.dims) {
        if (dim == 0) return false;
    }
    return true;
}

#ifdef _WIN32
bool compute_strides(const std::vector<uint32_t>& sizes, std::vector<uint32_t>& strides) {
    if (sizes.empty()) return false;
    strides.assign(sizes.size(), 0);
    uint64_t stride = 1;
    for (size_t idx = sizes.size(); idx-- > 0;) {
        if (stride > UINT32_MAX) return false;
        strides[idx] = static_cast<uint32_t>(stride);
        stride *= sizes[idx];
    }
    return true;
}

bool compute_total_bytes(const std::vector<uint32_t>& sizes, uint32_t element_size, size_t& out) {
    if (element_size == 0) return false;
    size_t total = 1;
    for (auto dim : sizes) {
        if (!safe_mul_size(total, dim, total)) return false;
    }
    return safe_mul_size(total, element_size, out);
}

bool build_buffer_tensor(const DmlTensorSpec& spec,
                         DML_TENSOR_DATA_TYPE dtype,
                         DmlGraph::BufferTensor& out) {
    if (!validate_tensor_spec(spec)) return false;
    out.sizes = spec.dims;
    if (!compute_strides(out.sizes, out.strides)) return false;

    size_t bytes = 0;
    if (!compute_total_bytes(out.sizes, spec.element_size, bytes)) return false;
    out.buffer_desc = {};
    out.buffer_desc.DataType = dtype;
    out.buffer_desc.Flags = DML_TENSOR_FLAG_NONE;
    out.buffer_desc.DimensionCount = static_cast<uint32_t>(out.sizes.size());
    out.buffer_desc.Sizes = out.sizes.data();
    out.buffer_desc.Strides = out.strides.data();
    out.buffer_desc.TotalTensorSizeInBytes = bytes;
    out.buffer_desc.GuaranteedBaseOffsetAlignment = 0;

    out.tensor_desc = {};
    out.tensor_desc.Type = DML_TENSOR_TYPE_BUFFER;
    out.tensor_desc.Desc = &out.buffer_desc;
    return true;
}

bool build_dml_graph_desc(const DmlTensorSpec& token_ids,
                          const DmlTensorSpec& logits,
                          const DmlTensorSpec& kv_cache,
                          DmlGraph::GraphDesc& desc) {
    if (token_ids.element_size != sizeof(uint32_t)) return false;
    if (logits.element_size != sizeof(float)) return false;
    if (kv_cache.element_size != sizeof(float)) return false;

    if (!build_buffer_tensor(token_ids, DML_TENSOR_DATA_TYPE_UINT32, desc.token_ids)) return false;
    if (!build_buffer_tensor(logits, DML_TENSOR_DATA_TYPE_FLOAT32, desc.logits)) return false;
    if (!build_buffer_tensor(kv_cache, DML_TENSOR_DATA_TYPE_FLOAT32, desc.kv_cache)) return false;
    desc.initialized = true;
    return true;
}

bool prepare_dml_operator_descs(DmlGraph& graph) {
    if (!graph.desc.initialized) return false;
    graph.prefill_desc.identity_desc = {};
    graph.prefill_desc.identity_desc.InputTensor = &graph.desc.logits.tensor_desc;
    graph.prefill_desc.identity_desc.OutputTensor = &graph.desc.logits.tensor_desc;
    graph.prefill_desc.op_desc = {};
    graph.prefill_desc.op_desc.Type = DML_OPERATOR_ELEMENT_WISE_IDENTITY;
    graph.prefill_desc.op_desc.Desc = &graph.prefill_desc.identity_desc;
    graph.prefill_desc.prepared = true;

    graph.decode_desc.identity_desc = {};
    graph.decode_desc.identity_desc.InputTensor = &graph.desc.logits.tensor_desc;
    graph.decode_desc.identity_desc.OutputTensor = &graph.desc.logits.tensor_desc;
    graph.decode_desc.op_desc = {};
    graph.decode_desc.op_desc.Type = DML_OPERATOR_ELEMENT_WISE_IDENTITY;
    graph.decode_desc.op_desc.Desc = &graph.decode_desc.identity_desc;
    graph.decode_desc.prepared = true;

    return true;
}
#endif

bool build_dml_graph_stub(const DmlTensorLayout& layout, DmlGraph& graph) {
    if (layout.vocab_size == 0 ||
        layout.embedding_dim == 0 ||
        layout.context_length == 0 ||
        layout.num_blocks == 0 ||
        layout.num_kv_heads == 0 ||
        layout.head_dim == 0) {
        return false;
    }
    DmlTensorSpec token_ids{{1}, sizeof(uint32_t)};
    DmlTensorSpec logits{{1, layout.vocab_size}, sizeof(float)};
    DmlTensorSpec kv_cache{
        {layout.num_blocks, layout.context_length, layout.num_kv_heads, layout.head_dim, 2},
        sizeof(float)};

    graph.layout = layout;
    graph.stub_graph = true;
    graph.prefill_inputs = {token_ids};
    graph.prefill_outputs = {logits, kv_cache};
    graph.decode_inputs = {token_ids, kv_cache};
    graph.decode_outputs = {logits, kv_cache};
#ifdef _WIN32
    if (!build_dml_graph_desc(token_ids, logits, kv_cache, graph.desc)) return false;
    if (!prepare_dml_operator_descs(graph)) return false;
    graph.initialized = graph.desc.initialized;
#endif
    for (const auto& spec : graph.prefill_inputs) {
        if (!validate_tensor_spec(spec)) return false;
    }
    for (const auto& spec : graph.prefill_outputs) {
        if (!validate_tensor_spec(spec)) return false;
    }
    for (const auto& spec : graph.decode_inputs) {
        if (!validate_tensor_spec(spec)) return false;
    }
    for (const auto& spec : graph.decode_outputs) {
        if (!validate_tensor_spec(spec)) return false;
    }
    graph.has_prefill = false;
    graph.has_decode = false;
    graph.initialized = false;
    return true;
}

bool dml_graph_ready(const DmlGraph& graph) {
    return graph.has_prefill && graph.has_decode;
}

bool compile_dml_operators(DmlGraph& graph, DmlExecState& exec_state) {
#ifdef _WIN32
    if (!exec_state.initialized || !exec_state.dml_device) return false;
    if (!graph.desc.initialized) return false;
    if (!graph.prefill_desc.prepared || !graph.decode_desc.prepared) return false;
    graph.prefill_op.Reset();
    graph.decode_op.Reset();

    ComPtr<IDMLOperator> prefill_op;
    if (FAILED(exec_state.dml_device->CreateOperator(&graph.prefill_desc.op_desc, IID_PPV_ARGS(&prefill_op)))) {
        return false;
    }
    if (FAILED(exec_state.dml_device->CompileOperator(prefill_op.Get(), DML_EXECUTION_FLAG_NONE,
                                                      IID_PPV_ARGS(&graph.prefill_op)))) {
        return false;
    }

    ComPtr<IDMLOperator> decode_op;
    if (FAILED(exec_state.dml_device->CreateOperator(&graph.decode_desc.op_desc, IID_PPV_ARGS(&decode_op)))) {
        return false;
    }
    if (FAILED(exec_state.dml_device->CompileOperator(decode_op.Get(), DML_EXECUTION_FLAG_NONE,
                                                      IID_PPV_ARGS(&graph.decode_op)))) {
        return false;
    }

    graph.has_prefill = graph.prefill_op != nullptr;
    graph.has_decode = graph.decode_op != nullptr;
    return graph.has_prefill && graph.has_decode;
#else
    (void)graph;
    (void)exec_state;
    return false;
#endif
}

gptoss_status run_dml_prefill(GptossContext* ctx) {
    if (!ctx || !ctx->model) return gptoss_status_invalid_argument;
    auto* model = reinterpret_cast<GptossModel*>(ctx->model);
    if (!uuid_equals(model->layout_uuid, kDirectMlLayoutUuid)) {
        return gptoss_status_unsupported_system;
    }
    if (ctx->tokens.size() * sizeof(uint32_t) > ctx->dml_plan.token_buffer_bytes) {
        return gptoss_status_context_overflow;
    }
#ifdef _WIN32
    if (model->dml_graph.stub_graph) {
        set_stub_logits(*ctx, model->dml_layout.vocab_size);
        return gptoss_status_success;
    }
#endif
    if (!model->dml_graph.has_prefill) {
        compile_dml_operators(model->dml_graph, ctx->dml_exec);
    }
    if (!dml_graph_ready(model->dml_graph)) {
        return gptoss_status_unsupported_argument;
    }
    if (!ctx->dml_buffers.initialized || !ctx->dml_exec.initialized || !ctx->dml_bindings.initialized) {
        return gptoss_status_insufficient_resources;
    }
#ifdef _WIN32
    if (!upload_tokens_to_gpu(ctx->dml_exec, ctx->tokens, ctx->dml_buffers)) {
        return gptoss_status_internal;
    }
    if (!reset_dml_command_list(ctx->dml_exec)) return gptoss_status_internal;
    if (ctx->dml_exec.binding_table) {
        ctx->dml_exec.binding_table->BindInputs(1, &ctx->dml_bindings.token_binding_desc);
        ctx->dml_exec.binding_table->BindOutputs(
            static_cast<UINT>(ctx->dml_bindings.prefill_output_descs.size()),
            ctx->dml_bindings.prefill_output_descs.data());
        ctx->dml_exec.binding_table_for_decode = false;
    }
    if (ctx->dml_exec.recorder && model->dml_graph.prefill_op) {
        ctx->dml_exec.recorder->RecordDispatch(
            ctx->dml_exec.command_list.Get(),
            model->dml_graph.prefill_op.Get(),
            ctx->dml_exec.binding_table.Get(),
            nullptr);
    }
    if (!submit_dml_command_list(ctx->dml_exec)) return gptoss_status_internal;
    std::vector<float> logits;
    if (!read_logits_from_gpu(ctx->dml_exec, model->dml_layout.vocab_size, logits, ctx->dml_buffers)) {
        return gptoss_status_internal;
    }
    ctx->last_logits = std::move(logits);
    ctx->logits_ready = !ctx->last_logits.empty();
    return gptoss_status_success;
#else
    return gptoss_status_unsupported_system;
#endif
}

gptoss_status run_dml_decode(GptossContext* ctx) {
    if (!ctx || !ctx->model) return gptoss_status_invalid_argument;
    auto* model = reinterpret_cast<GptossModel*>(ctx->model);
    if (!uuid_equals(model->layout_uuid, kDirectMlLayoutUuid)) {
        return gptoss_status_unsupported_system;
    }
    if (ctx->tokens.size() * sizeof(uint32_t) > ctx->dml_plan.token_buffer_bytes) {
        return gptoss_status_context_overflow;
    }
    if (static_cast<size_t>(model->dml_layout.vocab_size) * sizeof(float) > ctx->dml_plan.logits_buffer_bytes) {
        return gptoss_status_insufficient_resources;
    }
#ifdef _WIN32
    if (model->dml_graph.stub_graph) {
        set_stub_logits(*ctx, model->dml_layout.vocab_size);
        return gptoss_status_success;
    }
#endif
    if (!model->dml_graph.has_decode) {
        compile_dml_operators(model->dml_graph, ctx->dml_exec);
    }
    if (!dml_graph_ready(model->dml_graph)) {
        return gptoss_status_unsupported_argument;
    }
    if (!ctx->dml_buffers.initialized || !ctx->dml_exec.initialized || !ctx->dml_bindings.initialized) {
        return gptoss_status_insufficient_resources;
    }
#ifdef _WIN32
    if (!upload_tokens_to_gpu(ctx->dml_exec, ctx->tokens, ctx->dml_buffers)) {
        return gptoss_status_internal;
    }
    if (!reset_dml_command_list(ctx->dml_exec)) return gptoss_status_internal;
    if (ctx->dml_exec.binding_table) {
        if (!ctx->dml_exec.binding_table_for_decode) {
            ctx->dml_exec.binding_table->BindInputs(
                static_cast<UINT>(ctx->dml_bindings.decode_input_descs.size()),
                ctx->dml_bindings.decode_input_descs.data());
            ctx->dml_exec.binding_table->BindOutputs(1, &ctx->dml_bindings.logits_binding_desc);
            ctx->dml_exec.binding_table_for_decode = true;
        }
    }
    if (ctx->dml_exec.recorder && model->dml_graph.decode_op) {
        ctx->dml_exec.recorder->RecordDispatch(
            ctx->dml_exec.command_list.Get(),
            model->dml_graph.decode_op.Get(),
            ctx->dml_exec.binding_table.Get(),
            nullptr);
    }
    if (!submit_dml_command_list(ctx->dml_exec)) return gptoss_status_internal;
    std::vector<float> logits;
    if (!read_logits_from_gpu(ctx->dml_exec, model->dml_layout.vocab_size, logits, ctx->dml_buffers)) {
        return gptoss_status_internal;
    }
    ctx->last_logits = std::move(logits);
    ctx->logits_ready = !ctx->last_logits.empty();
    return gptoss_status_success;
#else
    return gptoss_status_unsupported_system;
#endif
}

bool safe_mul_size(size_t a, size_t b, size_t& out) {
    if (a == 0 || b == 0) {
        out = 0;
        return true;
    }
    if (a > SIZE_MAX / b) return false;
    out = a * b;
    return true;
}

bool build_dml_context_plan(const GptossModelHeader& header,
                            uint32_t vocabulary_size,
                            size_t max_tokens,
                            size_t max_batch_tokens,
                            DmlContextPlan& plan) {
    if (max_tokens == 0 || max_batch_tokens == 0) return false;
    if (max_batch_tokens > max_tokens) return false;

    size_t token_bytes = 0;
    if (!safe_mul_size(max_tokens, sizeof(uint32_t), token_bytes)) return false;
    plan.token_buffer_bytes = token_bytes;

    size_t logits_elements = 0;
    if (!safe_mul_size(max_batch_tokens, static_cast<size_t>(vocabulary_size), logits_elements)) return false;
    if (!safe_mul_size(logits_elements, sizeof(float), plan.logits_buffer_bytes)) return false;

    size_t kv_elements = 0;
    if (!safe_mul_size(static_cast<size_t>(header.num_blocks), max_tokens, kv_elements)) return false;
    if (!safe_mul_size(kv_elements, static_cast<size_t>(header.num_kv_heads), kv_elements)) return false;
    if (!safe_mul_size(kv_elements, static_cast<size_t>(header.head_dim), kv_elements)) return false;
    if (!safe_mul_size(kv_elements, static_cast<size_t>(2), kv_elements)) return false;
    if (!safe_mul_size(kv_elements, sizeof(float), plan.kv_cache_bytes)) return false;

    return true;
}

gptoss_status load_gptoss_model_file(const fs::path& path,
                                     GptossModelHeader& model_header,
                                     GptossUuid& layout_uuid,
                                     struct GptossTokenizer& tokenizer,
                                     size_t* weights_offset_out,
                                     size_t* weights_bytes_out) {
    std::ifstream in(path, std::ios::binary);
    if (!in) return gptoss_status_io_error;

    GptossFileHeader header{};
    if (!read_exact(in, &header, sizeof(header))) return gptoss_status_io_error;
    if (std::memcmp(header.magic, "GPT-OSS v1.0", sizeof(header.magic)) != 0 || header.zero != 0) {
        return gptoss_status_invalid_argument;
    }

    static const std::array<uint8_t, 16> kModelUuid = {
        0xDF, 0x52, 0xDC, 0x86, 0x17, 0x89, 0x4E, 0xD0, 0xA2, 0x95, 0x66, 0xF1, 0x05, 0x08, 0x14, 0x5B,
    };
    static const std::array<uint8_t, 16> kTokenizerUuid = {
        0x74, 0x01, 0xAD, 0xED, 0x2A, 0x95, 0x40, 0xCB, 0xB7, 0x82, 0x9C, 0xCE, 0xBA, 0xAF, 0xE7, 0x2B,
    };
    static const std::array<uint8_t, 16> kAppleGpuLayoutUuid = {
        0x22, 0x91, 0x77, 0xA8, 0x57, 0x75, 0x42, 0x68, 0xBF, 0xD8, 0xD5, 0x88, 0xB3, 0x51, 0xC5, 0x6D,
    };
    GptossUuid model_uuid{};
    if (!read_exact(in, &model_uuid, sizeof(model_uuid))) return gptoss_status_io_error;
    if (!uuid_equals(model_uuid, kModelUuid)) return gptoss_status_invalid_argument;

    if (!read_exact(in, &model_header, sizeof(model_header))) return gptoss_status_io_error;

    if (!read_exact(in, &layout_uuid, sizeof(layout_uuid))) return gptoss_status_io_error;
    if (uuid_equals(layout_uuid, kAppleGpuLayoutUuid)) return gptoss_status_unsupported_argument;
    if (!uuid_equals(layout_uuid, kDirectMlLayoutUuid)) return gptoss_status_unsupported_argument;

    GptossUuid tokenizer_uuid{};
    if (!read_exact(in, &tokenizer_uuid, sizeof(tokenizer_uuid))) return gptoss_status_io_error;
    if (!uuid_equals(tokenizer_uuid, kTokenizerUuid)) return gptoss_status_invalid_argument;

    GptossTokenizerHeader tok_header{};
    if (!read_exact(in, &tok_header, sizeof(tok_header))) return gptoss_status_io_error;

    tokenizer.special_ids.fill(UINT32_MAX);
    tokenizer.num_special_tokens = tok_header.num_special_tokens;
    tokenizer.num_text_tokens = tok_header.num_text_tokens;

    for (uint32_t idx = 0; idx < tok_header.num_special_tokens; ++idx) {
        GptossUuid token_uuid{};
        if (!read_exact(in, &token_uuid, sizeof(token_uuid))) return gptoss_status_io_error;
        const auto token_type = decode_special_token_uuid(token_uuid);
        if (token_type != gptoss_special_token_invalid) {
            tokenizer.special_ids[token_type - 1] = tok_header.num_text_tokens + idx;
        }
    }

    if (tok_header.regex_size != 0) {
        in.seekg(static_cast<std::streamoff>(tok_header.regex_size), std::ios::cur);
        if (!in.good()) return gptoss_status_io_error;
    }

    if (tok_header.tokens_size != 0) {
        tokenizer.tokens_blob.resize(tok_header.tokens_size);
        if (!read_exact(in, tokenizer.tokens_blob.data(), tok_header.tokens_size)) return gptoss_status_io_error;
    } else {
        return gptoss_status_invalid_argument;
    }
    const auto weights_offset = in.tellg();
    if (weights_offset == std::streampos(-1)) return gptoss_status_io_error;
    if (weights_offset_out || weights_bytes_out) {
        std::error_code ec;
        const auto file_size = fs::file_size(path, ec);
        if (ec) return gptoss_status_io_error;
        if (file_size < static_cast<uintmax_t>(weights_offset)) return gptoss_status_io_error;
        const size_t weights_bytes = static_cast<size_t>(file_size - static_cast<uintmax_t>(weights_offset));
        if (weights_offset_out) *weights_offset_out = static_cast<size_t>(weights_offset);
        if (weights_bytes_out) *weights_bytes_out = weights_bytes;
    }

    const uint8_t* ptr = tokenizer.tokens_blob.data();
    const uint8_t* end = ptr + tokenizer.tokens_blob.size();
    for (uint32_t t = 0; t < tokenizer.num_text_tokens; ++t) {
        if (ptr + sizeof(uint16_t) > end) return gptoss_status_invalid_argument;
        uint16_t len = 0;
        std::memcpy(&len, ptr, sizeof(len));
        ptr += sizeof(uint16_t);
        if (ptr + len > end) return gptoss_status_invalid_argument;
        ptr += len;
    }

    if (model_header.context_length == 0 ||
        model_header.num_blocks == 0 ||
        model_header.embedding_dim == 0 ||
        model_header.num_heads == 0 ||
        model_header.num_kv_heads == 0 ||
        model_header.head_dim == 0) {
        return gptoss_status_invalid_argument;
    }
    if (model_header.num_kv_heads > model_header.num_heads) {
        return gptoss_status_invalid_argument;
    }
    return gptoss_status_success;
}

#ifdef _WIN32
struct DmlRuntime {
    ComPtr<IDXGIAdapter1> adapter;
    ComPtr<ID3D12Device> device;
    ComPtr<IDMLDevice> dml_device;
    HMODULE dml_module{nullptr};
    bool initialized{false};
    std::string error;
};

static DmlRuntime& dml_runtime() {
    static DmlRuntime runtime;
    return runtime;
}

static std::once_flag& dml_runtime_once() {
    static std::once_flag once;
    return once;
}

bool init_dml_runtime(DmlRuntime& runtime) {
    ComPtr<IDXGIFactory6> factory;
    HRESULT hr = CreateDXGIFactory1(IID_PPV_ARGS(&factory));
    if (FAILED(hr)) {
        runtime.error = "DirectML: failed to create DXGI factory";
        return false;
    }

    ComPtr<IDXGIAdapter1> adapter;
    for (UINT index = 0; factory->EnumAdapterByGpuPreference(index, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
                                                             IID_PPV_ARGS(&adapter)) != DXGI_ERROR_NOT_FOUND; ++index) {
        DXGI_ADAPTER_DESC1 desc = {};
        if (FAILED(adapter->GetDesc1(&desc))) continue;
        if (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE) continue;
        runtime.adapter = adapter;
        break;
    }

    if (!runtime.adapter) {
        runtime.error = "DirectML: no compatible GPU adapter found";
        return false;
    }

    hr = D3D12CreateDevice(runtime.adapter.Get(), D3D_FEATURE_LEVEL_12_0, IID_PPV_ARGS(&runtime.device));
    if (FAILED(hr)) {
        runtime.error = "DirectML: failed to create D3D12 device";
        return false;
    }

    runtime.dml_module = LoadLibraryW(L"DirectML.dll");
    if (!runtime.dml_module) {
        runtime.error = "DirectML: DirectML.dll not found";
        return false;
    }

    using DmlCreateDeviceFn =
        HRESULT(WINAPI*)(ID3D12Device*, DML_CREATE_DEVICE_FLAGS, REFIID, void**);
    using DmlCreateDevice1Fn =
        HRESULT(WINAPI*)(ID3D12Device*, DML_CREATE_DEVICE_FLAGS, DML_FEATURE_LEVEL, REFIID, void**);

    auto create1_fn = reinterpret_cast<DmlCreateDevice1Fn>(
        GetProcAddress(runtime.dml_module, "DMLCreateDevice1"));
    auto create_fn = reinterpret_cast<DmlCreateDeviceFn>(
        GetProcAddress(runtime.dml_module, "DMLCreateDevice"));
    if (create1_fn) {
        hr = create1_fn(runtime.device.Get(),
                        DML_CREATE_DEVICE_FLAG_NONE,
                        DML_FEATURE_LEVEL_1_0,
                        IID_PPV_ARGS(&runtime.dml_device));
        if (FAILED(hr) && create_fn) {
            hr = create_fn(runtime.device.Get(),
                           DML_CREATE_DEVICE_FLAG_NONE,
                           IID_PPV_ARGS(&runtime.dml_device));
        }
    } else if (create_fn) {
        hr = create_fn(runtime.device.Get(),
                       DML_CREATE_DEVICE_FLAG_NONE,
                       IID_PPV_ARGS(&runtime.dml_device));
    } else {
        runtime.error = "DirectML: DMLCreateDevice not exported";
        return false;
    }

    if (FAILED(hr)) {
        runtime.error = "DirectML: failed to create DML device";
        return false;
    }

    runtime.initialized = true;
    return true;
}

bool ensure_dml_runtime(std::string& error) {
    auto& runtime = dml_runtime();
    std::call_once(dml_runtime_once(), [&]() { runtime.initialized = init_dml_runtime(runtime); });
    if (!runtime.initialized) {
        error = runtime.error;
        return false;
    }
    return true;
}

bool create_dml_buffer(ID3D12Device* device, size_t bytes, ComPtr<ID3D12Resource>& out) {
    if (!device || bytes == 0) return false;
    D3D12_HEAP_PROPERTIES heap_props = {};
    heap_props.Type = D3D12_HEAP_TYPE_DEFAULT;
    heap_props.CPUPageProperty = D3D12_CPU_PAGE_PROPERTY_UNKNOWN;
    heap_props.MemoryPoolPreference = D3D12_MEMORY_POOL_UNKNOWN;
    heap_props.CreationNodeMask = 1;
    heap_props.VisibleNodeMask = 1;

    D3D12_RESOURCE_DESC desc = {};
    desc.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
    desc.Alignment = 0;
    desc.Width = static_cast<UINT64>(bytes);
    desc.Height = 1;
    desc.DepthOrArraySize = 1;
    desc.MipLevels = 1;
    desc.Format = DXGI_FORMAT_UNKNOWN;
    desc.SampleDesc.Count = 1;
    desc.SampleDesc.Quality = 0;
    desc.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
    desc.Flags = D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;

    HRESULT hr = device->CreateCommittedResource(
        &heap_props,
        D3D12_HEAP_FLAG_NONE,
        &desc,
        D3D12_RESOURCE_STATE_COMMON,
        nullptr,
        IID_PPV_ARGS(out.GetAddressOf()));
    return SUCCEEDED(hr);
}

bool create_dml_staging_buffer(ID3D12Device* device,
                               size_t bytes,
                               D3D12_HEAP_TYPE heap_type,
                               D3D12_RESOURCE_STATES initial_state,
                               ComPtr<ID3D12Resource>& out) {
    if (!device || bytes == 0) return false;
    D3D12_HEAP_PROPERTIES heap_props = {};
    heap_props.Type = heap_type;
    heap_props.CPUPageProperty = D3D12_CPU_PAGE_PROPERTY_UNKNOWN;
    heap_props.MemoryPoolPreference = D3D12_MEMORY_POOL_UNKNOWN;
    heap_props.CreationNodeMask = 1;
    heap_props.VisibleNodeMask = 1;

    D3D12_RESOURCE_DESC desc = {};
    desc.Dimension = D3D12_RESOURCE_DIMENSION_BUFFER;
    desc.Alignment = 0;
    desc.Width = static_cast<UINT64>(bytes);
    desc.Height = 1;
    desc.DepthOrArraySize = 1;
    desc.MipLevels = 1;
    desc.Format = DXGI_FORMAT_UNKNOWN;
    desc.SampleDesc.Count = 1;
    desc.SampleDesc.Quality = 0;
    desc.Layout = D3D12_TEXTURE_LAYOUT_ROW_MAJOR;
    desc.Flags = D3D12_RESOURCE_FLAG_NONE;

    HRESULT hr = device->CreateCommittedResource(
        &heap_props,
        D3D12_HEAP_FLAG_NONE,
        &desc,
        initial_state,
        nullptr,
        IID_PPV_ARGS(out.GetAddressOf()));
    return SUCCEEDED(hr);
}

bool upload_weights_to_gpu(const GptossModel& model, ComPtr<ID3D12Resource>& out) {
    if (!model.has_weights_blob || model.model_path.empty()) return false;
    std::ifstream in(model.model_path, std::ios::binary);
    if (!in) return false;
    in.seekg(static_cast<std::streamoff>(model.weights_offset), std::ios::beg);
    if (!in.good()) return false;

    std::vector<uint8_t> payload(model.weights_bytes);
    if (!read_exact(in, payload.data(), payload.size())) return false;

    auto& runtime = dml_runtime();
    if (!runtime.initialized || !runtime.device) return false;
    if (!create_dml_buffer(runtime.device.Get(), model.weights_bytes, out)) return false;

    ComPtr<ID3D12Resource> upload;
    if (!create_dml_staging_buffer(runtime.device.Get(),
                                   model.weights_bytes,
                                   D3D12_HEAP_TYPE_UPLOAD,
                                   D3D12_RESOURCE_STATE_GENERIC_READ,
                                   upload)) {
        return false;
    }

    void* mapped = nullptr;
    D3D12_RANGE range = {0, 0};
    if (FAILED(upload->Map(0, &range, &mapped))) return false;
    std::memcpy(mapped, payload.data(), payload.size());
    upload->Unmap(0, nullptr);

    DmlExecState exec;
    if (!init_dml_exec_state(exec)) return false;
    if (!reset_dml_command_list(exec)) return false;
    exec.command_list->CopyBufferRegion(out.Get(), 0, upload.Get(), 0, payload.size());
    if (!submit_dml_command_list(exec)) return false;
    return true;
}

bool init_dml_buffers(const DmlContextPlan& plan, DmlBuffers& buffers) {
    std::string error;
    if (!ensure_dml_runtime(error)) {
        return false;
    }
    auto& runtime = dml_runtime();
    if (!runtime.initialized || !runtime.device) {
        return false;
    }
    if (!create_dml_buffer(runtime.device.Get(), plan.token_buffer_bytes, buffers.token_buffer)) return false;
    if (!create_dml_buffer(runtime.device.Get(), plan.logits_buffer_bytes, buffers.logits_buffer)) return false;
    if (!create_dml_buffer(runtime.device.Get(), plan.kv_cache_bytes, buffers.kv_cache_buffer)) return false;
    if (!create_dml_staging_buffer(runtime.device.Get(),
                                   plan.token_buffer_bytes,
                                   D3D12_HEAP_TYPE_UPLOAD,
                                   D3D12_RESOURCE_STATE_GENERIC_READ,
                                   buffers.token_upload_buffer)) {
        return false;
    }
    if (!create_dml_staging_buffer(runtime.device.Get(),
                                   plan.logits_buffer_bytes,
                                   D3D12_HEAP_TYPE_READBACK,
                                   D3D12_RESOURCE_STATE_COPY_DEST,
                                   buffers.logits_readback_buffer)) {
        return false;
    }
    buffers.initialized = true;
    return true;
}

bool init_dml_bindings(const DmlGraph::GraphDesc& desc,
                       const DmlBuffers& buffers,
                       DmlBindings& bindings) {
    if (!desc.initialized) return false;
    if (!buffers.initialized) return false;
    if (!buffers.token_buffer || !buffers.logits_buffer || !buffers.kv_cache_buffer) return false;

    bindings.token_binding.Buffer = buffers.token_buffer.Get();
    bindings.token_binding.Offset = 0;
    bindings.token_binding.SizeInBytes = desc.token_ids.buffer_desc.TotalTensorSizeInBytes;

    bindings.logits_binding.Buffer = buffers.logits_buffer.Get();
    bindings.logits_binding.Offset = 0;
    bindings.logits_binding.SizeInBytes = desc.logits.buffer_desc.TotalTensorSizeInBytes;

    bindings.kv_cache_binding.Buffer = buffers.kv_cache_buffer.Get();
    bindings.kv_cache_binding.Offset = 0;
    bindings.kv_cache_binding.SizeInBytes = desc.kv_cache.buffer_desc.TotalTensorSizeInBytes;

    bindings.token_binding_desc.Type = DML_BINDING_TYPE_BUFFER;
    bindings.token_binding_desc.Desc = &bindings.token_binding;
    bindings.logits_binding_desc.Type = DML_BINDING_TYPE_BUFFER;
    bindings.logits_binding_desc.Desc = &bindings.logits_binding;
    bindings.kv_cache_binding_desc.Type = DML_BINDING_TYPE_BUFFER;
    bindings.kv_cache_binding_desc.Desc = &bindings.kv_cache_binding;

    bindings.decode_input_descs = {bindings.token_binding_desc, bindings.kv_cache_binding_desc};
    bindings.prefill_output_descs = {bindings.logits_binding_desc, bindings.kv_cache_binding_desc};

    bindings.initialized = true;
    return true;
}

bool init_dml_exec_state(DmlExecState& state) {
    std::string error;
    if (!ensure_dml_runtime(error)) {
        return false;
    }
    auto& runtime = dml_runtime();
    if (!runtime.initialized || !runtime.device || !runtime.dml_device) {
        return false;
    }
    state.dml_device = runtime.dml_device;

    D3D12_COMMAND_QUEUE_DESC queue_desc = {};
    queue_desc.Type = D3D12_COMMAND_LIST_TYPE_DIRECT;
    queue_desc.Priority = D3D12_COMMAND_QUEUE_PRIORITY_NORMAL;
    queue_desc.Flags = D3D12_COMMAND_QUEUE_FLAG_NONE;
    queue_desc.NodeMask = 0;
    if (FAILED(runtime.device->CreateCommandQueue(&queue_desc, IID_PPV_ARGS(&state.command_queue)))) {
        return false;
    }
    if (FAILED(runtime.device->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT,
                                                      IID_PPV_ARGS(&state.allocator)))) {
        return false;
    }
    if (FAILED(runtime.device->CreateCommandList(0,
                                                D3D12_COMMAND_LIST_TYPE_DIRECT,
                                                state.allocator.Get(),
                                                nullptr,
                                                IID_PPV_ARGS(&state.command_list)))) {
        return false;
    }
    if (FAILED(state.command_list->Close())) {
        return false;
    }
    if (FAILED(runtime.device->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&state.fence)))) {
        return false;
    }
    state.fence_event = CreateEvent(nullptr, FALSE, FALSE, nullptr);
    if (!state.fence_event) {
        return false;
    }
    if (FAILED(DMLCreateCommandRecorder(IID_PPV_ARGS(&state.recorder)))) {
        return false;
    }
    state.initialized = true;
    return true;
}

bool init_dml_binding_table(DmlExecState& state,
                            const DmlGraph& graph,
                            const DmlBindings& bindings) {
    if (!state.initialized || !state.dml_device) return false;
    if (!graph.prefill_op || !graph.decode_op) return false;
    if (!bindings.initialized) return false;

    DML_BINDING_TABLE_DESC table_desc = {};
    table_desc.Dispatchable = graph.prefill_op.Get();
    table_desc.CPUDescriptorHandle = {};
    table_desc.GPUDescriptorHandle = {};
    table_desc.SizeInDescriptors = 2;

    state.binding_table.Reset();
    if (FAILED(state.dml_device->CreateBindingTable(&table_desc, IID_PPV_ARGS(&state.binding_table)))) {
        return false;
    }

    state.binding_table->BindInputs(1, &bindings.token_binding_desc);
    state.binding_table->BindOutputs(2, bindings.prefill_output_descs.data());
    state.binding_table_for_decode = false;
    return true;
}

bool reset_dml_command_list(DmlExecState& state) {
    if (!state.initialized) return false;
    if (FAILED(state.allocator->Reset())) return false;
    if (FAILED(state.command_list->Reset(state.allocator.Get(), nullptr))) return false;
    return true;
}

bool submit_dml_command_list(DmlExecState& state) {
    if (!state.initialized) return false;
    if (FAILED(state.command_list->Close())) return false;
    ID3D12CommandList* lists[] = {state.command_list.Get()};
    state.command_queue->ExecuteCommandLists(1, lists);
    state.fence_value += 1;
    if (FAILED(state.command_queue->Signal(state.fence.Get(), state.fence_value))) return false;
    if (state.fence->GetCompletedValue() < state.fence_value) {
        if (FAILED(state.fence->SetEventOnCompletion(state.fence_value, state.fence_event))) return false;
        WaitForSingleObject(state.fence_event, INFINITE);
    }
    return true;
}

bool upload_tokens_to_gpu(DmlExecState& state,
                          const std::vector<uint32_t>& tokens,
                          DmlBuffers& buffers) {
    if (!state.initialized || !state.command_list || !state.command_queue || !state.allocator) return false;
    if (!buffers.token_buffer || !buffers.token_upload_buffer) return false;
    const size_t bytes = tokens.size() * sizeof(uint32_t);
    if (bytes == 0) return true;

    void* mapped = nullptr;
    D3D12_RANGE range = {0, 0};
    if (FAILED(buffers.token_upload_buffer->Map(0, &range, &mapped))) {
        return false;
    }
    std::memcpy(mapped, tokens.data(), bytes);
    buffers.token_upload_buffer->Unmap(0, nullptr);

    if (!reset_dml_command_list(state)) return false;
    state.command_list->CopyBufferRegion(
        buffers.token_buffer.Get(), 0, buffers.token_upload_buffer.Get(), 0, bytes);
    if (!submit_dml_command_list(state)) return false;
    return true;
}

bool read_logits_from_gpu(DmlExecState& state,
                          size_t vocab_size,
                          std::vector<float>& logits,
                          DmlBuffers& buffers) {
    if (!state.initialized || !buffers.logits_buffer || !buffers.logits_readback_buffer) return false;
    if (vocab_size == 0) return false;
    const size_t bytes = vocab_size * sizeof(float);

    if (!reset_dml_command_list(state)) return false;
    state.command_list->CopyBufferRegion(
        buffers.logits_readback_buffer.Get(), 0, buffers.logits_buffer.Get(), 0, bytes);
    if (!submit_dml_command_list(state)) return false;

    logits.resize(vocab_size);
    void* mapped = nullptr;
    D3D12_RANGE range = {0, static_cast<SIZE_T>(bytes)};
    if (FAILED(buffers.logits_readback_buffer->Map(0, &range, &mapped))) {
        return false;
    }
    std::memcpy(logits.data(), mapped, bytes);
    buffers.logits_readback_buffer->Unmap(0, nullptr);
    return true;
}
#endif

struct GptossContext {
    std::atomic<uint32_t> ref_count{1};
    gptoss_model_t model{nullptr};
    std::vector<uint32_t> tokens;
    size_t max_tokens{0};
    DmlContextPlan dml_plan{};
    DmlBuffers dml_buffers{};
    DmlBindings dml_bindings{};
    DmlExecState dml_exec{};
    std::vector<float> last_logits;
    bool logits_ready{false};
    uint64_t rng_state{0};
    bool rng_initialized{false};
    bool prefill_done{false};
};

uint64_t next_rng(uint64_t& state) {
    if (state == 0) {
        state = UINT64_C(0x9e3779b97f4a7c15);
    }
    state ^= state >> 12;
    state ^= state << 25;
    state ^= state >> 27;
    return state * UINT64_C(2685821657736338717);
}

double rng_uniform_01(uint64_t& state) {
    const uint64_t value = next_rng(state);
    return static_cast<double>(value >> 11) * (1.0 / 9007199254740992.0);
}

uint32_t select_stub_token(const GptossContext& ctx, uint32_t vocab_size) {
    if (vocab_size == 0) return 0;
    if (!ctx.tokens.empty()) {
        const uint32_t last = ctx.tokens.back();
        if (last < vocab_size) return last;
    }
    return 0;
}

void set_stub_logits(GptossContext& ctx, uint32_t vocab_size) {
    ctx.last_logits.assign(vocab_size, 0.0f);
    const uint32_t token = select_stub_token(ctx, vocab_size);
    if (!ctx.last_logits.empty()) {
        ctx.last_logits[token] = 1.0f;
    }
    ctx.logits_ready = !ctx.last_logits.empty();
}

bool sample_from_logits(const std::vector<float>& logits,
                        float temperature,
                        uint64_t& rng_state,
                        uint32_t& token_out) {
    if (logits.empty()) return false;
    auto max_it = std::max_element(logits.begin(), logits.end());
    if (max_it == logits.end()) return false;
    const float max_logit = *max_it;
    if (!(temperature > 0.0f)) {
        token_out = static_cast<uint32_t>(std::distance(logits.begin(), max_it));
        return true;
    }

    double sum = 0.0;
    for (float logit : logits) {
        sum += std::exp(static_cast<double>(logit - max_logit) * static_cast<double>(temperature));
    }
    if (!(sum > 0.0)) return false;

    const double target = rng_uniform_01(rng_state) * sum;
    double cumulative = 0.0;
    for (size_t i = 0; i < logits.size(); ++i) {
        cumulative += std::exp(static_cast<double>(logits[i] - max_logit) * static_cast<double>(temperature));
        if (cumulative >= target) {
            token_out = static_cast<uint32_t>(i);
            return true;
        }
    }

    token_out = static_cast<uint32_t>(std::distance(logits.begin(), max_it));
    return true;
}

uint32_t find_unknown_token_id(const GptossTokenizer& tokenizer) {
    static const char* kUnknownTokens[] = {"<unk>", "<|unk|>", "<|unknown|>"};
    for (const char* candidate : kUnknownTokens) {
        auto it = tokenizer.token_to_id.find(candidate);
        if (it != tokenizer.token_to_id.end()) return it->second;
    }
    return UINT32_MAX;
}

bool parse_vocab(const nlohmann::json& model, GptossTokenizer& tokenizer) {
    if (!model.is_object()) return false;
    auto vocab_it = model.find("vocab");
    if (vocab_it == model.end() || !vocab_it->is_object()) return false;

    size_t max_id = 0;
    for (const auto& item : vocab_it->items()) {
        if (!item.value().is_number_unsigned()) continue;
        max_id = std::max(max_id, static_cast<size_t>(item.value().get<uint32_t>()));
    }

    tokenizer.id_to_token.assign(max_id + 1, "");
    for (const auto& item : vocab_it->items()) {
        if (!item.value().is_number_unsigned()) continue;
        const uint32_t id = item.value().get<uint32_t>();
        if (id >= tokenizer.id_to_token.size()) continue;
        tokenizer.id_to_token[id] = item.key();
        tokenizer.token_to_id[item.key()] = id;
    }

    return !tokenizer.id_to_token.empty();
}

void map_special_tokens(GptossTokenizer& tokenizer) {
    tokenizer.special_ids.fill(UINT32_MAX);
    const std::pair<const char*, gptoss_special_token> mapping[] = {
        {"<|return|>", gptoss_special_token_return},
        {"<|start|>", gptoss_special_token_start},
        {"<|message|>", gptoss_special_token_message},
        {"<|end|>", gptoss_special_token_end},
        {"<|refusal|>", gptoss_special_token_refusal},
        {"<|constrain|>", gptoss_special_token_constrain},
        {"<|channel|>", gptoss_special_token_channel},
        {"<|call|>", gptoss_special_token_call},
        {"<|untrusted|>", gptoss_special_token_untrusted},
        {"<|end_untrusted|>", gptoss_special_token_end_untrusted},
    };

    for (const auto& entry : mapping) {
        auto it = tokenizer.token_to_id.find(entry.first);
        if (it != tokenizer.token_to_id.end()) {
            tokenizer.special_ids[entry.second - 1] = it->second;
            tokenizer.num_special_tokens += 1;
        }
    }
}

bool load_tokenizer(const fs::path& tokenizer_path, GptossTokenizer& tokenizer) {
    std::ifstream in(tokenizer_path);
    if (!in) return false;

    nlohmann::json json;
    in >> json;

    const auto model_it = json.find("model");
    if (model_it == json.end() || !parse_vocab(*model_it, tokenizer)) return false;

    map_special_tokens(tokenizer);
    tokenizer.num_text_tokens = static_cast<uint32_t>(
        tokenizer.id_to_token.size() - tokenizer.num_special_tokens);
    return true;
}

uint32_t load_max_context_length(const fs::path& config_path) {
    std::ifstream in(config_path);
    if (!in) return 0;

    nlohmann::json json;
    in >> json;
    for (const auto& key : {"max_position_embeddings", "context_length"}) {
        auto it = json.find(key);
        if (it != json.end() && it->is_number_unsigned()) {
            return it->get<uint32_t>();
        }
    }
    return 0;
}

bool is_directml_model_bin(const fs::path& path) {
    auto name = path.filename().string();
    std::transform(name.begin(), name.end(), name.begin(),
                   [](unsigned char ch) { return static_cast<char>(std::tolower(ch)); });
    return name == "model.directml.bin" || name == "model.dml.bin";
}

bool tokenizer_uses_blob(const GptossTokenizer& tokenizer) {
    return !tokenizer.tokens_blob.empty();
}

gptoss_status decode_blob_token(const GptossTokenizer& tokenizer,
                                uint32_t token,
                                const char** token_ptr_out,
                                size_t* token_size_out) {
    if (token >= tokenizer.num_text_tokens) return gptoss_status_invalid_argument;
    const uint8_t* ptr = tokenizer.tokens_blob.data();
    const uint8_t* end = ptr + tokenizer.tokens_blob.size();
    for (uint32_t t = 0; t < tokenizer.num_text_tokens; ++t) {
        if (ptr + sizeof(uint16_t) > end) return gptoss_status_invalid_argument;
        uint16_t len = 0;
        std::memcpy(&len, ptr, sizeof(len));
        ptr += sizeof(uint16_t);
        if (ptr + len > end) return gptoss_status_invalid_argument;
        if (t == token) {
            *token_ptr_out = reinterpret_cast<const char*>(ptr);
            *token_size_out = len;
            return gptoss_status_success;
        }
        ptr += len;
    }
    return gptoss_status_invalid_argument;
}

gptoss_status tokenize_blob(const GptossTokenizer& tokenizer,
                            const char* text,
                            size_t size,
                            std::vector<uint32_t>& out) {
    size_t remaining = size;
    const char* current = text;
    while (remaining != 0) {
        uint32_t best_token = UINT32_MAX;
        uint32_t best_length = 0;
        const uint8_t* ptr = tokenizer.tokens_blob.data();
        const uint8_t* end = ptr + tokenizer.tokens_blob.size();
        for (uint32_t t = 0; t < tokenizer.num_text_tokens; ++t) {
            if (ptr + sizeof(uint16_t) > end) return gptoss_status_invalid_argument;
            uint16_t len = 0;
            std::memcpy(&len, ptr, sizeof(len));
            ptr += sizeof(uint16_t);
            if (ptr + len > end) return gptoss_status_invalid_argument;
            if (len <= remaining && len > best_length) {
                if (std::memcmp(current, ptr, len) == 0) {
                    best_token = t;
                    best_length = len;
                }
            }
            ptr += len;
        }
        if (best_token == UINT32_MAX) return gptoss_status_invalid_argument;
        out.push_back(best_token);
        current += best_length;
        remaining -= best_length;
    }
    return gptoss_status_success;
}

gptoss_status tokenize_into(const GptossTokenizer& tokenizer, const char* text, size_t size, std::vector<uint32_t>& out) {
    if (tokenizer_uses_blob(tokenizer)) {
        return tokenize_blob(tokenizer, text, size, out);
    }
    const uint32_t unknown_id = find_unknown_token_id(tokenizer);
    std::string current;
    current.reserve(16);

    auto flush = [&]() -> gptoss_status {
        if (current.empty()) return gptoss_status_success;
        auto it = tokenizer.token_to_id.find(current);
        if (it != tokenizer.token_to_id.end()) {
            out.push_back(it->second);
        } else if (unknown_id != UINT32_MAX) {
            out.push_back(unknown_id);
        } else {
            return gptoss_status_invalid_argument;
        }
        current.clear();
        return gptoss_status_success;
    };

    for (size_t i = 0; i < size; i++) {
        const unsigned char ch = static_cast<unsigned char>(text[i]);
        if (std::isspace(ch)) {
            auto st = flush();
            if (st != gptoss_status_success) return st;
            std::string space(1, static_cast<char>(ch));
            auto it = tokenizer.token_to_id.find(space);
            if (it != tokenizer.token_to_id.end()) {
                out.push_back(it->second);
            } else if (unknown_id != UINT32_MAX) {
                out.push_back(unknown_id);
            } else {
                return gptoss_status_invalid_argument;
            }
        } else {
            current.push_back(static_cast<char>(ch));
        }
    }

    return flush();
}

}  // namespace

extern "C" {

gptoss_status GPTOSS_ABI gptoss_model_create_from_file(
    const char* model_path,
    gptoss_model_t* model_out) {
    if (!model_out) return gptoss_status_invalid_argument;
    *model_out = nullptr;
    if (!model_path || model_path[0] == '\0') return gptoss_status_invalid_argument;

    fs::path path(model_path);
    fs::path model_dir = fs::is_directory(path) ? path : path.parent_path();
    if (model_dir.empty()) return gptoss_status_invalid_argument;

#ifdef _WIN32
    std::string dml_error;
    if (!ensure_dml_runtime(dml_error)) {
        return gptoss_status_unsupported_system;
    }
#endif

    if (is_directml_model_bin(path)) {
        auto model = new (std::nothrow) GptossModel();
        if (!model) return gptoss_status_insufficient_memory;
        model->model_dir = model_dir.string();

        auto tokenizer = new (std::nothrow) GptossTokenizer();
        if (!tokenizer) {
            delete model;
            return gptoss_status_insufficient_memory;
        }
        GptossUuid layout_uuid{};
        size_t weights_offset = 0;
        size_t weights_bytes = 0;
        auto status = load_gptoss_model_file(path, model->header, layout_uuid, *tokenizer,
                                             &weights_offset, &weights_bytes);
        if (status != gptoss_status_success) {
            delete tokenizer;
            delete model;
            return status;
        }

        model->max_context_length = model->header.context_length;
        model->vocabulary_size = tokenizer->num_text_tokens + tokenizer->num_special_tokens;
        model->layout_uuid = layout_uuid;
        model->model_path = path.string();
        model->weights_offset = weights_offset;
        model->weights_bytes = weights_bytes;
        model->has_weights_blob = weights_bytes > 0;
        if (uuid_equals(layout_uuid, kDirectMlLayoutUuid)) {
            build_dml_plan(model->header, model->vocabulary_size, model->dml_plan);
            build_dml_tensor_layout(model->header, model->vocabulary_size, model->dml_layout);
            build_dml_graph_stub(model->dml_layout, model->dml_graph);
#ifdef _WIN32
            if (model->has_weights_blob && !model->weights_buffer) {
                if (!upload_weights_to_gpu(*model, model->weights_buffer)) {
                    delete tokenizer;
                    delete model;
                    return gptoss_status_insufficient_resources;
                }
            }
#endif
        }
        model->tokenizer = reinterpret_cast<gptoss_tokenizer_t>(tokenizer);
        *model_out = reinterpret_cast<gptoss_model_t>(model);
        return gptoss_status_success;
    }

    const fs::path config_path = model_dir / "config.json";
    const fs::path tokenizer_path = model_dir / "tokenizer.json";
    if (!fs::exists(config_path) || !fs::exists(tokenizer_path)) {
        return gptoss_status_io_error;
    }

    auto model = new (std::nothrow) GptossModel();
    if (!model) return gptoss_status_insufficient_memory;
    model->model_dir = model_dir.string();
    model->max_context_length = load_max_context_length(config_path);

    auto tokenizer = new (std::nothrow) GptossTokenizer();
    if (!tokenizer) {
        delete model;
        return gptoss_status_insufficient_memory;
    }
    if (!load_tokenizer(tokenizer_path, *tokenizer)) {
        delete tokenizer;
        delete model;
        return gptoss_status_io_error;
    }

    model->vocabulary_size = static_cast<uint32_t>(tokenizer->id_to_token.size());
    model->tokenizer = reinterpret_cast<gptoss_tokenizer_t>(tokenizer);
    *model_out = reinterpret_cast<gptoss_model_t>(model);
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_model_get_tokenizer(
    gptoss_model_t model,
    gptoss_tokenizer_t* tokenizer_out) {
    if (!model || !tokenizer_out) return gptoss_status_invalid_argument;
    *tokenizer_out = reinterpret_cast<GptossModel*>(model)->tokenizer;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_model_get_max_context_length(
    gptoss_model_t model,
    uint32_t* max_context_length_out) {
    if (!model || !max_context_length_out) return gptoss_status_invalid_argument;
    *max_context_length_out = reinterpret_cast<GptossModel*>(model)->max_context_length;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_model_retain(gptoss_model_t model) {
    if (!model) return gptoss_status_invalid_argument;
    reinterpret_cast<GptossModel*>(model)->ref_count.fetch_add(1, std::memory_order_relaxed);
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_model_release(gptoss_model_t model) {
    if (!model) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossModel*>(model);
    if (ptr->ref_count.fetch_sub(1, std::memory_order_acq_rel) == 1) {
        if (ptr->tokenizer) {
            gptoss_tokenizer_release(ptr->tokenizer);
        }
        delete ptr;
    }
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_get_special_token_id(
    gptoss_tokenizer_t tokenizer,
    enum gptoss_special_token token_type,
    uint32_t* token_id_out) {
    if (!tokenizer || !token_id_out) return gptoss_status_invalid_argument;
    if (token_type <= gptoss_special_token_invalid || token_type >= gptoss_special_token_max) {
        return gptoss_status_invalid_argument;
    }
    auto* ptr = reinterpret_cast<GptossTokenizer*>(tokenizer);
    const uint32_t id = ptr->special_ids[token_type - 1];
    if (id == UINT32_MAX) return gptoss_status_invalid_argument;
    *token_id_out = id;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_get_num_text_tokens(
    gptoss_tokenizer_t tokenizer,
    uint32_t* num_text_tokens_out) {
    if (!tokenizer || !num_text_tokens_out) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossTokenizer*>(tokenizer);
    if (tokenizer_uses_blob(*ptr)) {
        *num_text_tokens_out = ptr->num_text_tokens;
    } else {
        *num_text_tokens_out = static_cast<uint32_t>(ptr->id_to_token.size() - ptr->num_special_tokens);
    }
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_get_num_special_tokens(
    gptoss_tokenizer_t tokenizer,
    uint32_t* num_special_tokens_out) {
    if (!tokenizer || !num_special_tokens_out) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossTokenizer*>(tokenizer);
    *num_special_tokens_out = ptr->num_special_tokens;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_get_num_tokens(
    gptoss_tokenizer_t tokenizer,
    uint32_t* num_tokens_out) {
    if (!tokenizer || !num_tokens_out) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossTokenizer*>(tokenizer);
    if (tokenizer_uses_blob(*ptr)) {
        *num_tokens_out = ptr->num_text_tokens + ptr->num_special_tokens;
    } else {
        *num_tokens_out = static_cast<uint32_t>(ptr->id_to_token.size());
    }
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_decode(
    gptoss_tokenizer_t tokenizer,
    uint32_t token,
    const char** token_ptr_out,
    size_t* token_size_out) {
    if (!tokenizer || !token_ptr_out || !token_size_out) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossTokenizer*>(tokenizer);
    if (tokenizer_uses_blob(*ptr)) {
        return decode_blob_token(*ptr, token, token_ptr_out, token_size_out);
    }
    if (token >= ptr->id_to_token.size()) return gptoss_status_invalid_argument;
    const std::string& tok = ptr->id_to_token[token];
    *token_ptr_out = tok.c_str();
    *token_size_out = tok.size();
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_retain(gptoss_tokenizer_t tokenizer) {
    if (!tokenizer) return gptoss_status_invalid_argument;
    reinterpret_cast<GptossTokenizer*>(tokenizer)->ref_count.fetch_add(1, std::memory_order_relaxed);
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_tokenizer_release(gptoss_tokenizer_t tokenizer) {
    if (!tokenizer) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossTokenizer*>(tokenizer);
    if (ptr->ref_count.fetch_sub(1, std::memory_order_acq_rel) == 1) {
        delete ptr;
    }
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_create(
    gptoss_model_t model,
    size_t context_length,
    size_t max_batch_tokens,
    gptoss_context_t* context_out) {
    if (!context_out || !model) return gptoss_status_invalid_argument;
    *context_out = nullptr;

    auto* model_ptr = reinterpret_cast<GptossModel*>(model);
    const size_t max_tokens = context_length == 0 ? model_ptr->max_context_length : context_length;
    if (max_tokens == 0) return gptoss_status_invalid_argument;
    if (max_batch_tokens != 0 && max_batch_tokens > max_tokens) {
        return gptoss_status_invalid_argument;
    }

    auto* ctx = new (std::nothrow) GptossContext();
    if (!ctx) return gptoss_status_insufficient_memory;
    ctx->model = model;
    ctx->max_tokens = max_tokens;
    auto* model_ptr = reinterpret_cast<GptossModel*>(model);
    if (uuid_equals(model_ptr->layout_uuid, kDirectMlLayoutUuid)) {
        if (!build_dml_context_plan(model_ptr->header,
                                    model_ptr->vocabulary_size,
                                    ctx->max_tokens,
                                    max_batch_tokens == 0 ? ctx->max_tokens : max_batch_tokens,
                                    ctx->dml_plan)) {
            delete ctx;
            return gptoss_status_invalid_argument;
        }
        if (!init_dml_buffers(ctx->dml_plan, ctx->dml_buffers)) {
            delete ctx;
            return gptoss_status_insufficient_resources;
        }
#ifdef _WIN32
        if (!init_dml_bindings(model_ptr->dml_graph.desc, ctx->dml_buffers, ctx->dml_bindings)) {
            delete ctx;
            return gptoss_status_insufficient_resources;
        }
#endif
        if (!init_dml_exec_state(ctx->dml_exec)) {
            delete ctx;
            return gptoss_status_insufficient_resources;
        }
#ifdef _WIN32
        if (!init_dml_binding_table(ctx->dml_exec, model_ptr->dml_graph, ctx->dml_bindings)) {
            delete ctx;
            return gptoss_status_insufficient_resources;
        }
#endif
    }
    gptoss_model_retain(model);
    *context_out = reinterpret_cast<gptoss_context_t>(ctx);
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_get_num_tokens(
    gptoss_context_t context,
    size_t* num_tokens_out) {
    if (!context || !num_tokens_out) return gptoss_status_invalid_argument;
    *num_tokens_out = reinterpret_cast<GptossContext*>(context)->tokens.size();
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_get_max_tokens(
    gptoss_context_t context,
    size_t* max_tokens_out) {
    if (!context || !max_tokens_out) return gptoss_status_invalid_argument;
    *max_tokens_out = reinterpret_cast<GptossContext*>(context)->max_tokens;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_get_tokens(
    gptoss_context_t context,
    uint32_t* tokens_out,
    size_t max_tokens,
    size_t* num_tokens_out) {
    if (!context || !num_tokens_out) return gptoss_status_invalid_argument;
    auto* ctx = reinterpret_cast<GptossContext*>(context);
    *num_tokens_out = ctx->tokens.size();
    if (max_tokens < ctx->tokens.size()) return gptoss_status_insufficient_memory;
    if (tokens_out && !ctx->tokens.empty()) {
        std::copy(ctx->tokens.begin(), ctx->tokens.end(), tokens_out);
    }
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_append_chars(
    gptoss_context_t context,
    const char* text,
    size_t text_size,
    size_t* num_tokens_out) {
    if (!context || !text) return gptoss_status_invalid_argument;
    auto* ctx = reinterpret_cast<GptossContext*>(context);
    auto* model = reinterpret_cast<GptossModel*>(ctx->model);
    auto* tokenizer = reinterpret_cast<GptossTokenizer*>(model->tokenizer);

    std::vector<uint32_t> new_tokens;
    auto st = tokenize_into(*tokenizer, text, text_size, new_tokens);
    if (st != gptoss_status_success) return st;

    if (ctx->tokens.size() + new_tokens.size() > ctx->max_tokens) {
        return gptoss_status_context_overflow;
    }
    ctx->tokens.insert(ctx->tokens.end(), new_tokens.begin(), new_tokens.end());
    ctx->last_logits.clear();
    ctx->logits_ready = false;
    ctx->prefill_done = false;
    if (num_tokens_out) *num_tokens_out = new_tokens.size();
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_append_tokens(
    gptoss_context_t context,
    size_t num_tokens,
    const uint32_t* tokens) {
    if (!context || (num_tokens != 0 && !tokens)) return gptoss_status_invalid_argument;
    auto* ctx = reinterpret_cast<GptossContext*>(context);
    auto* model = reinterpret_cast<GptossModel*>(ctx->model);
    if (model && model->vocabulary_size != 0) {
        for (size_t i = 0; i < num_tokens; ++i) {
            if (tokens[i] >= model->vocabulary_size) {
                return gptoss_status_invalid_argument;
            }
        }
    }
    if (ctx->tokens.size() + num_tokens > ctx->max_tokens) {
        return gptoss_status_context_overflow;
    }
    ctx->tokens.insert(ctx->tokens.end(), tokens, tokens + num_tokens);
    ctx->last_logits.clear();
    ctx->logits_ready = false;
    ctx->prefill_done = false;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_reset(gptoss_context_t context) {
    if (!context) return gptoss_status_invalid_argument;
    auto* ctx = reinterpret_cast<GptossContext*>(context);
    ctx->tokens.clear();
    ctx->last_logits.clear();
    ctx->logits_ready = false;
    ctx->prefill_done = false;
    ctx->rng_state = 0;
    ctx->rng_initialized = false;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_process(gptoss_context_t context) {
    if (!context) return gptoss_status_invalid_argument;
    auto* ctx = reinterpret_cast<GptossContext*>(context);
    ctx->last_logits.clear();
    ctx->logits_ready = false;
    ctx->prefill_done = false;
    const auto status = run_dml_prefill(ctx);
    if (status == gptoss_status_success) {
        ctx->prefill_done = true;
    }
    return status;
}

gptoss_status GPTOSS_ABI gptoss_context_sample(
    gptoss_context_t context,
    float temperature,
    uint64_t rng_state,
    size_t num_tokens,
    uint32_t* tokens_out,
    size_t* num_tokens_out) {
    if (!context) return gptoss_status_invalid_argument;
    auto* ctx = reinterpret_cast<GptossContext*>(context);
    if (num_tokens == 0) {
        if (num_tokens_out) *num_tokens_out = 0;
        return gptoss_status_success;
    }
    if (!tokens_out || !num_tokens_out) return gptoss_status_invalid_argument;
    if (!ctx->model) return gptoss_status_invalid_argument;
    if (ctx->tokens.size() >= ctx->max_tokens) return gptoss_status_context_overflow;
    if (!ctx->rng_initialized) {
        ctx->rng_state = rng_state == 0 ? UINT64_C(0x9e3779b97f4a7c15) : rng_state;
        ctx->rng_initialized = true;
    }

    const float effective_temperature = temperature < 0.0f ? 0.0f : temperature;
    size_t produced = 0;
    if (!ctx->prefill_done && !ctx->tokens.empty()) {
        const auto status = run_dml_prefill(ctx);
        if (status != gptoss_status_success) return status;
        ctx->prefill_done = true;
    }
    for (size_t i = 0; i < num_tokens; ++i) {
        if (!ctx->logits_ready) {
            const auto status = run_dml_decode(ctx);
            if (status != gptoss_status_success) return status;
        }
        if (!ctx->logits_ready || ctx->last_logits.empty()) return gptoss_status_internal;

        uint32_t token = 0;
        if (!sample_from_logits(ctx->last_logits, effective_temperature, ctx->rng_state, token)) {
            return gptoss_status_internal;
        }
        if (ctx->tokens.size() >= ctx->max_tokens) return gptoss_status_context_overflow;
        ctx->tokens.push_back(token);
        tokens_out[i] = token;
        produced++;
        ctx->logits_ready = false;
    }
    *num_tokens_out = produced;
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_retain(gptoss_context_t context) {
    if (!context) return gptoss_status_invalid_argument;
    reinterpret_cast<GptossContext*>(context)->ref_count.fetch_add(1, std::memory_order_relaxed);
    return gptoss_status_success;
}

gptoss_status GPTOSS_ABI gptoss_context_release(gptoss_context_t context) {
    if (!context) return gptoss_status_invalid_argument;
    auto* ptr = reinterpret_cast<GptossContext*>(context);
    if (ptr->ref_count.fetch_sub(1, std::memory_order_acq_rel) == 1) {
        if (ptr->model) {
            gptoss_model_release(ptr->model);
        }
#ifdef _WIN32
        if (ptr->dml_exec.fence_event) {
            CloseHandle(ptr->dml_exec.fence_event);
            ptr->dml_exec.fence_event = nullptr;
        }
#endif
        delete ptr;
    }
    return gptoss_status_success;
}

}  // extern "C"
