#include <iostream>
#include <fstream>
#include <sstream>
#include <string>
#include <cstdlib>
#include "config.h"
#include "inference.h"

#ifdef _WIN32
#include <windows.h>
#endif

namespace {

struct Args {
    std::string model_path;
    std::string prompt;
    std::string prompt_file;
    size_t max_tokens = 100;
    int device_id = 0;
    bool verbose = false;
    bool help = false;
};

void printUsage(const char* program) {
    std::cerr << "Nemotron CUDA PoC - Direct safetensors to CUDA inference\n\n";
    std::cerr << "Usage: " << program << " --model PATH --prompt TEXT [options]\n\n";
    std::cerr << "Required:\n";
    std::cerr << "  --model PATH       Path to model directory (with config.json, tokenizer.json, *.safetensors)\n";
    std::cerr << "  --prompt TEXT      Input prompt for generation\n";
    std::cerr << "  --prompt-file FILE Read prompt from UTF-8 file (for non-ASCII input)\n\n";
    std::cerr << "Options:\n";
    std::cerr << "  --max-tokens N     Maximum tokens to generate (default: 100)\n";
    std::cerr << "  --device N         CUDA device ID (default: 0)\n";
    std::cerr << "  --verbose          Enable verbose output\n";
    std::cerr << "  --help             Show this help message\n\n";
    std::cerr << "Example:\n";
    std::cerr << "  " << program << " --model /path/to/nemotron-mini --prompt \"Hello, world!\"\n";
    std::cerr << "  " << program << " --model /path/to/model --prompt-file prompt.txt\n";
}

Args parseArgs(int argc, char* argv[]) {
    Args args;

    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];

        if (arg == "--help" || arg == "-h") {
            args.help = true;
        } else if (arg == "--model") {
            if (i + 1 >= argc) {
                std::cerr << "Error: --model requires a path\n";
                std::exit(1);
            }
            args.model_path = argv[++i];
        } else if (arg == "--prompt") {
            if (i + 1 >= argc) {
                std::cerr << "Error: --prompt requires text\n";
                std::exit(1);
            }
            args.prompt = argv[++i];
        } else if (arg == "--prompt-file") {
            if (i + 1 >= argc) {
                std::cerr << "Error: --prompt-file requires a file path\n";
                std::exit(1);
            }
            args.prompt_file = argv[++i];
        } else if (arg == "--max-tokens") {
            if (i + 1 >= argc) {
                std::cerr << "Error: --max-tokens requires a number\n";
                std::exit(1);
            }
            args.max_tokens = std::stoul(argv[++i]);
        } else if (arg == "--device") {
            if (i + 1 >= argc) {
                std::cerr << "Error: --device requires a number\n";
                std::exit(1);
            }
            args.device_id = std::stoi(argv[++i]);
        } else if (arg == "--verbose") {
            args.verbose = true;
        } else {
            std::cerr << "Error: Unknown argument: " << arg << "\n";
            printUsage(argv[0]);
            std::exit(1);
        }
    }

    return args;
}

}  // namespace

int main(int argc, char* argv[]) {
#ifdef _WIN32
    // Set console to UTF-8
    SetConsoleOutputCP(CP_UTF8);
#endif

    Args args = parseArgs(argc, argv);

    if (args.help) {
        printUsage(argv[0]);
        return 0;
    }

    // Validate required arguments
    if (args.model_path.empty()) {
        std::cerr << "Error: --model is required\n\n";
        printUsage(argv[0]);
        return 1;
    }

    // Load prompt from file if specified
    if (!args.prompt_file.empty()) {
        std::ifstream file(args.prompt_file);
        if (!file) {
            std::cerr << "Error: Cannot open prompt file: " << args.prompt_file << "\n";
            return 1;
        }
        std::stringstream buffer;
        buffer << file.rdbuf();
        args.prompt = buffer.str();
        // Trim trailing newline/carriage return
        while (!args.prompt.empty() && (args.prompt.back() == '\n' || args.prompt.back() == '\r')) {
            args.prompt.pop_back();
        }
        if (args.prompt.empty()) {
            std::cerr << "Error: Prompt file is empty: " << args.prompt_file << "\n";
            return 1;
        }
    }

    if (args.prompt.empty()) {
        std::cerr << "Error: --prompt or --prompt-file is required\n\n";
        printUsage(argv[0]);
        return 1;
    }

    try {
        // Check CUDA availability
        if (!nemotron::checkCudaAvailable()) {
            std::cerr << "[ERROR] CUDA is not available on this system\n";
            std::cerr << "        Please ensure you have a CUDA-capable GPU and the CUDA driver is installed.\n";
            return 1;
        }

        // Create inference engine
        nemotron::InferenceEngine engine;

        // Load model
        engine.loadModel(args.model_path, args.device_id);

        // Configure generation
        nemotron::GenerationConfig gen_config;
        gen_config.max_tokens = args.max_tokens;
        gen_config.greedy = true;  // Use greedy decoding for PoC

        std::cout << "\n=== Generation ===\n";
        std::cout << "Prompt: " << args.prompt << "\n";
        std::cout << "Output: ";

        // Generate
        std::string output = engine.generate(args.prompt, gen_config);

        // Print statistics
        const auto& stats = engine.getStats();
        std::cout << "\n=== Statistics ===\n";
        std::cout << "Load time:       " << stats.load_time_ms << " ms\n";
        std::cout << "Prompt tokens:   " << stats.prompt_tokens << "\n";
        std::cout << "Prompt time:     " << stats.prompt_time_ms << " ms\n";
        std::cout << "Generated:       " << stats.generated_tokens << " tokens\n";
        std::cout << "Generation time: " << stats.generation_time_ms << " ms\n";
        std::cout << "Speed:           " << stats.tokensPerSecond() << " tokens/sec\n";

        return 0;

    } catch (const nemotron::FileError& e) {
        std::cerr << "[ERROR] File error: " << e.what() << "\n";
        std::cerr << "        Hint: Check that the model path is correct and contains all required files.\n";
        return 1;
    } catch (const nemotron::CudaError& e) {
        std::cerr << "[ERROR] CUDA error: " << e.what() << "\n";
        std::cerr << "        Hint: Ensure your GPU has enough memory and CUDA drivers are up to date.\n";
        return 1;
    } catch (const nemotron::ModelError& e) {
        std::cerr << "[ERROR] Model error: " << e.what() << "\n";
        std::cerr << "        Hint: Verify the model format is compatible with Nemotron architecture.\n";
        return 1;
    } catch (const std::exception& e) {
        std::cerr << "[ERROR] Unexpected error: " << e.what() << "\n";
        return 1;
    }
}
