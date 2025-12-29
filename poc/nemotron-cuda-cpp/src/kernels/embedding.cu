#include "kernels.h"
#include "cuda_utils.h"

namespace nemotron {
namespace kernels {

__global__ void embeddingLookupKernel(
    __nv_bfloat16* __restrict__ output,
    const __nv_bfloat16* __restrict__ embed_table,
    const int32_t* __restrict__ token_ids,
    size_t seq_len,
    size_t hidden_size
) {
    // Each block handles one token
    size_t token_pos = blockIdx.x;
    if (token_pos >= seq_len) return;

    int32_t token_id = token_ids[token_pos];
    const __nv_bfloat16* embed_row = embed_table + token_id * hidden_size;
    __nv_bfloat16* out_row = output + token_pos * hidden_size;

    // Copy embedding vector
    for (size_t i = threadIdx.x; i < hidden_size; i += blockDim.x) {
        out_row[i] = embed_row[i];
    }
}

void embeddingLookup(
    __nv_bfloat16* output,
    const __nv_bfloat16* embed_table,
    const int32_t* token_ids,
    size_t batch_size,
    size_t seq_len,
    size_t hidden_size,
    cudaStream_t stream
) {
    // For simplicity, handle batch_size=1 case
    // For batch > 1, would need to adjust indexing
    (void)batch_size;

    dim3 grid(seq_len);
    dim3 block(std::min(hidden_size, size_t(256)));

    embeddingLookupKernel<<<grid, block, 0, stream>>>(
        output, embed_table, token_ids, seq_len, hidden_size
    );
    CUDA_KERNEL_CHECK();
}

}  // namespace kernels
}  // namespace nemotron
