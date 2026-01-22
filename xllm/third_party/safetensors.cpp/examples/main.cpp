/**
 * @file main.cpp
 * @brief safetensors.cpp CLI example
 *
 * Usage:
 *   safetensors-cli --model ./model --prompt "Hello, world!" --max-tokens 100
 */

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include "safetensors.h"

void print_usage(const char* program) {
    printf("Usage: %s [options]\n", program);
    printf("\n");
    printf("Options:\n");
    printf("  --model <path>       Model directory path (required)\n");
    printf("  --prompt <text>      Input prompt (required)\n");
    printf("  --max-tokens <n>     Maximum tokens to generate (default: 100)\n");
    printf("  --temperature <f>    Temperature (default: 1.0)\n");
    printf("  --top-p <f>          Top-p sampling (default: 1.0)\n");
    printf("  --top-k <n>          Top-k sampling (default: -1, disabled)\n");
    printf("  --seed <n>           Random seed (default: -1, random)\n");
    printf("  --stream             Enable streaming output\n");
    printf("  --version            Show version and exit\n");
    printf("  --help               Show this help\n");
}

void print_version() {
    printf("safetensors.cpp v%s (ABI v%d)\n", stcpp_version(), stcpp_abi_version());

    // Print backend info
    printf("\nAvailable backends:\n");
    int32_t n = stcpp_n_backends();
    for (int32_t i = 0; i < n; i++) {
        stcpp_backend_type backend = stcpp_backend_type_at(i);
        printf("  - %s\n", stcpp_backend_name(backend));

        int32_t devices = stcpp_n_devices(backend);
        for (int32_t d = 0; d < devices; d++) {
            const char* name = stcpp_device_name(backend, d);
            size_t vram = stcpp_device_vram_total(backend, d);
            if (vram > 0) {
                printf("    [%d] %s (%.1f GB)\n", d, name, vram / (1024.0 * 1024.0 * 1024.0));
            } else {
                printf("    [%d] %s\n", d, name);
            }
        }
    }
}

bool stream_callback(const char* token, int32_t token_id, void* user_data) {
    (void)token_id;
    (void)user_data;
    printf("%s", token);
    fflush(stdout);
    return true;  // Continue generating
}

int main(int argc, char** argv) {
    // Parse arguments
    const char* model_path = nullptr;
    const char* prompt = nullptr;
    int32_t max_tokens = 100;
    float temperature = 1.0f;
    float top_p = 1.0f;
    int32_t top_k = -1;
    int32_t seed = -1;
    bool streaming = false;

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--help") == 0 || strcmp(argv[i], "-h") == 0) {
            print_usage(argv[0]);
            return 0;
        }
        if (strcmp(argv[i], "--version") == 0 || strcmp(argv[i], "-v") == 0) {
            print_version();
            return 0;
        }
        if (strcmp(argv[i], "--model") == 0 && i + 1 < argc) {
            model_path = argv[++i];
        } else if (strcmp(argv[i], "--prompt") == 0 && i + 1 < argc) {
            prompt = argv[++i];
        } else if (strcmp(argv[i], "--max-tokens") == 0 && i + 1 < argc) {
            max_tokens = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--temperature") == 0 && i + 1 < argc) {
            temperature = static_cast<float>(atof(argv[++i]));
        } else if (strcmp(argv[i], "--top-p") == 0 && i + 1 < argc) {
            top_p = static_cast<float>(atof(argv[++i]));
        } else if (strcmp(argv[i], "--top-k") == 0 && i + 1 < argc) {
            top_k = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--seed") == 0 && i + 1 < argc) {
            seed = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--stream") == 0) {
            streaming = true;
        }
    }

    // Validate arguments
    if (!model_path) {
        fprintf(stderr, "Error: --model is required\n");
        print_usage(argv[0]);
        return 1;
    }
    if (!prompt) {
        fprintf(stderr, "Error: --prompt is required\n");
        print_usage(argv[0]);
        return 1;
    }

    // Initialize library
    stcpp_init();

    // Load model
    printf("Loading model from: %s\n", model_path);
    stcpp_model* model = stcpp_model_load(model_path, nullptr, nullptr);
    if (!model) {
        fprintf(stderr, "Error: Failed to load model\n");
        stcpp_free();
        return 1;
    }

    // Create context
    stcpp_context_params ctx_params = stcpp_context_default_params();
    stcpp_context* ctx = stcpp_context_new(model, ctx_params);
    if (!ctx) {
        fprintf(stderr, "Error: Failed to create context\n");
        stcpp_model_free(model);
        stcpp_free();
        return 1;
    }

    // Set sampling parameters
    stcpp_sampling_params sampling = stcpp_sampling_default_params();
    sampling.temperature = temperature;
    sampling.top_p = top_p;
    sampling.top_k = top_k;
    sampling.seed = seed;

    printf("Prompt: %s\n", prompt);
    printf("Generating (max %d tokens)...\n\n", max_tokens);

    stcpp_error err;
    if (streaming) {
        // Streaming generation
        err = stcpp_generate_stream(ctx, prompt, sampling, max_tokens,
                                     stream_callback, nullptr);
        printf("\n");
    } else {
        // Synchronous generation
        char output[16384];
        err = stcpp_generate(ctx, prompt, sampling, max_tokens,
                             output, sizeof(output));
        if (err == STCPP_OK) {
            printf("%s\n", output);
        }
    }

    if (err != STCPP_OK) {
        fprintf(stderr, "\nError during generation: %d\n", err);
    }

    // Cleanup
    stcpp_context_free(ctx);
    stcpp_model_free(model);
    stcpp_free();

    return err == STCPP_OK ? 0 : 1;
}
