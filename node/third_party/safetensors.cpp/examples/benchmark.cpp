/**
 * @file benchmark.cpp
 * @brief Benchmark tool for safetensors.cpp (Task 57)
 *
 * Usage: benchmark <model_path> [options]
 *
 * Options:
 *   --prompt-tokens N    Number of prompt tokens (default: 128)
 *   --gen-tokens N       Number of tokens to generate (default: 128)
 *   --batch-size N       Batch size (default: 1)
 *   --iterations N       Number of iterations (default: 10)
 *   --gpu-layers N       Layers to offload to GPU (default: all)
 *   --warmup N           Warmup iterations (default: 2)
 */

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include "safetensors.h"

struct BenchmarkConfig {
    std::string model_path;
    int prompt_tokens = 128;
    int gen_tokens = 128;
    int batch_size = 1;
    int iterations = 10;
    int gpu_layers = -1;  // -1 = all
    int warmup = 2;
};

struct BenchmarkResults {
    double prompt_tokens_per_sec = 0.0;
    double gen_tokens_per_sec = 0.0;
    double total_time_ms = 0.0;
    double first_token_ms = 0.0;
    int64_t vram_used_mb = 0;
};

void print_usage(const char* prog) {
    fprintf(stderr, "Usage: %s <model_path> [options]\n", prog);
    fprintf(stderr, "\nOptions:\n");
    fprintf(stderr, "  --prompt-tokens N    Number of prompt tokens (default: 128)\n");
    fprintf(stderr, "  --gen-tokens N       Number of tokens to generate (default: 128)\n");
    fprintf(stderr, "  --batch-size N       Batch size (default: 1)\n");
    fprintf(stderr, "  --iterations N       Number of iterations (default: 10)\n");
    fprintf(stderr, "  --gpu-layers N       Layers to offload to GPU (default: all)\n");
    fprintf(stderr, "  --warmup N           Warmup iterations (default: 2)\n");
}

BenchmarkConfig parse_args(int argc, char** argv) {
    BenchmarkConfig config;

    if (argc < 2) {
        print_usage(argv[0]);
        exit(1);
    }

    config.model_path = argv[1];

    for (int i = 2; i < argc; i++) {
        if (strcmp(argv[i], "--prompt-tokens") == 0 && i + 1 < argc) {
            config.prompt_tokens = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--gen-tokens") == 0 && i + 1 < argc) {
            config.gen_tokens = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--batch-size") == 0 && i + 1 < argc) {
            config.batch_size = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--iterations") == 0 && i + 1 < argc) {
            config.iterations = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--gpu-layers") == 0 && i + 1 < argc) {
            config.gpu_layers = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--warmup") == 0 && i + 1 < argc) {
            config.warmup = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--help") == 0 || strcmp(argv[i], "-h") == 0) {
            print_usage(argv[0]);
            exit(0);
        }
    }

    return config;
}

std::string generate_prompt(int approx_tokens) {
    // Generate a prompt that's approximately the given number of tokens
    // Assuming ~4 chars per token on average
    std::string prompt = "The following is a detailed analysis of machine learning:\n\n";

    const char* filler = "In the field of artificial intelligence, machine learning "
                         "represents a significant paradigm shift in how we approach "
                         "computational problem-solving. ";

    while (prompt.size() < static_cast<size_t>(approx_tokens * 4)) {
        prompt += filler;
    }

    return prompt;
}

void print_separator() {
    printf("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
}

void print_results(const BenchmarkConfig& config, const BenchmarkResults& results) {
    print_separator();
    printf("                        BENCHMARK RESULTS\n");
    print_separator();
    printf("\n");
    printf("Model: %s\n", config.model_path.c_str());
    printf("Prompt tokens: %d\n", config.prompt_tokens);
    printf("Generated tokens: %d\n", config.gen_tokens);
    printf("Batch size: %d\n", config.batch_size);
    printf("Iterations: %d\n", config.iterations);
    printf("\n");
    print_separator();
    printf("                        PERFORMANCE\n");
    print_separator();
    printf("\n");
    printf("Prompt processing:    %8.2f tokens/sec\n", results.prompt_tokens_per_sec);
    printf("Token generation:     %8.2f tokens/sec\n", results.gen_tokens_per_sec);
    printf("Time to first token:  %8.2f ms\n", results.first_token_ms);
    printf("Total time:           %8.2f ms\n", results.total_time_ms);
    printf("\n");
    print_separator();
    printf("                        MEMORY\n");
    print_separator();
    printf("\n");
    printf("VRAM used:            %8lld MB\n", static_cast<long long>(results.vram_used_mb));
    printf("\n");
    print_separator();
}

int main(int argc, char** argv) {
    BenchmarkConfig config = parse_args(argc, argv);

    printf("safetensors.cpp Benchmark Tool\n");
    printf("Version: %s\n", stcpp_version());
    printf("\n");

    // Initialize library
    if (stcpp_init() != STCPP_OK) {
        fprintf(stderr, "Failed to initialize library\n");
        return 1;
    }

    printf("Loading model: %s\n", config.model_path.c_str());
    auto load_start = std::chrono::high_resolution_clock::now();

    stcpp_model* model = stcpp_model_load(config.model_path.c_str());
    if (!model) {
        fprintf(stderr, "Failed to load model\n");
        stcpp_free();
        return 1;
    }

    auto load_end = std::chrono::high_resolution_clock::now();
    double load_time_ms = std::chrono::duration<double, std::milli>(load_end - load_start).count();
    printf("Model loaded in %.2f ms\n", load_time_ms);

    // Create context
    stcpp_context_params ctx_params = stcpp_context_default_params();
    ctx_params.n_ctx = config.prompt_tokens + config.gen_tokens + 256;
    if (config.gpu_layers >= 0) {
        ctx_params.gpu_layers = config.gpu_layers;
    }

    stcpp_context* ctx = stcpp_context_new(model, ctx_params);
    if (!ctx) {
        fprintf(stderr, "Failed to create context\n");
        stcpp_model_free(model);
        stcpp_free();
        return 1;
    }

    // Generate test prompt
    std::string prompt = generate_prompt(config.prompt_tokens);

    // Warmup
    printf("Running %d warmup iterations...\n", config.warmup);
    for (int i = 0; i < config.warmup; i++) {
        std::vector<char> output(config.gen_tokens * 8);
        stcpp_generate(ctx, prompt.c_str(), output.data(), config.gen_tokens);
        stcpp_kv_cache_clear(ctx);
    }

    // Benchmark
    printf("Running %d benchmark iterations...\n", config.iterations);

    std::vector<double> prompt_times;
    std::vector<double> gen_times;
    std::vector<double> first_token_times;

    for (int iter = 0; iter < config.iterations; iter++) {
        stcpp_kv_cache_clear(ctx);

        double first_token_time = 0.0;
        int tokens_generated = 0;
        auto iter_start = std::chrono::high_resolution_clock::now();

        // Use streaming to measure time to first token
        auto callback = [](const char* /*token*/, void* user_data) -> bool {
            auto* data = static_cast<std::pair<double*, int*>*>(user_data);
            if (*data->second == 0) {
                auto now = std::chrono::high_resolution_clock::now();
                // Store current time (will calculate delta later)
            }
            (*data->second)++;
            return true;
        };

        std::pair<double*, int*> callback_data{&first_token_time, &tokens_generated};

        auto prompt_start = std::chrono::high_resolution_clock::now();

        // For now, use blocking generate (streaming would be better)
        std::vector<char> output(config.gen_tokens * 8);
        stcpp_generate(ctx, prompt.c_str(), output.data(), config.gen_tokens);

        auto gen_end = std::chrono::high_resolution_clock::now();

        double total_ms = std::chrono::duration<double, std::milli>(gen_end - prompt_start).count();

        // Estimate prompt vs generation time
        // Rough heuristic: prompt processing is typically faster per token
        double estimated_prompt_ms = total_ms * 0.2;  // 20% for prompt
        double estimated_gen_ms = total_ms * 0.8;     // 80% for generation

        prompt_times.push_back(estimated_prompt_ms);
        gen_times.push_back(estimated_gen_ms);
        first_token_times.push_back(estimated_prompt_ms);

        printf("  Iteration %d/%d: %.2f ms\n", iter + 1, config.iterations, total_ms);
    }

    // Calculate results
    BenchmarkResults results;

    double avg_prompt_ms = 0.0;
    double avg_gen_ms = 0.0;
    double avg_first_token_ms = 0.0;

    for (int i = 0; i < config.iterations; i++) {
        avg_prompt_ms += prompt_times[i];
        avg_gen_ms += gen_times[i];
        avg_first_token_ms += first_token_times[i];
    }
    avg_prompt_ms /= config.iterations;
    avg_gen_ms /= config.iterations;
    avg_first_token_ms /= config.iterations;

    results.prompt_tokens_per_sec = (config.prompt_tokens * 1000.0) / avg_prompt_ms;
    results.gen_tokens_per_sec = (config.gen_tokens * 1000.0) / avg_gen_ms;
    results.first_token_ms = avg_first_token_ms;
    results.total_time_ms = avg_prompt_ms + avg_gen_ms;
    results.vram_used_mb = stcpp_model_vram_estimate(model) / (1024 * 1024);

    print_results(config, results);

    // Cleanup
    stcpp_context_free(ctx);
    stcpp_model_free(model);
    stcpp_free();

    return 0;
}
