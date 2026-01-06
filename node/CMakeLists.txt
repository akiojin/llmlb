cmake_minimum_required(VERSION 3.20)
project(llm-node VERSION 1.0.0 LANGUAGES CXX C)

# Enable Objective-C/C++ on Apple platforms for Metal support
if(APPLE)
    enable_language(OBJC)
    enable_language(OBJCXX)
endif()

# Set C++ standard
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Export compile commands for IDE support
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Build type
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

# Options
option(PORTABLE_BUILD "Build with portable CPU flags for CI/cross-platform compatibility" OFF)

# Compiler flags
if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
    add_compile_options(-Wall -Wextra -Wpedantic)
    if(CMAKE_BUILD_TYPE STREQUAL "Debug")
        add_compile_options(-g -O0)
    else()
        add_compile_options(-O3)
        if(PORTABLE_BUILD)
            # Use baseline x86-64 for maximum compatibility
            add_compile_options(-march=x86-64 -mtune=generic)
        else()
            # Optimize for the build machine's CPU
            add_compile_options(-march=native)
        endif()
    endif()
elseif(MSVC)
    # Enable UTF-8 source and execution charset for Japanese comments
    # Use generator expression to apply only to C/C++ (not CUDA)
    add_compile_options($<$<COMPILE_LANGUAGE:C,CXX>:/utf-8>)
endif()

# Options
option(BUILD_TESTS "Build tests" ON)
option(BUILD_WITH_CUDA "Build with CUDA support" OFF)
option(BUILD_WITH_DIRECTML "Build with DirectML support (Windows, frozen)" OFF)
if(APPLE)
    option(BUILD_WITH_METAL "Build with Metal support (macOS)" ON)
    option(BUILD_WITH_SD_METAL "Build stable-diffusion with Metal backend (macOS)" ON)
else()
    option(BUILD_WITH_METAL "Build with Metal support (macOS)" OFF)
    option(BUILD_WITH_SD_METAL "Build stable-diffusion with Metal backend (macOS)" OFF)
endif()
if(APPLE AND BUILD_WITH_METAL)
    option(BUILD_WITH_GPTOSS "Build with OpenAI gpt-oss engine" ON)
elseif(WIN32)
    option(BUILD_WITH_GPTOSS "Build with OpenAI gpt-oss engine" ON)
else()
    option(BUILD_WITH_GPTOSS "Build with OpenAI gpt-oss engine" OFF)
endif()
option(BUILD_WITH_ROCM "Build with ROCm support" OFF)
option(STATIC_BUILD "Build statically linked executable" OFF)

# CUDA support (must be decided before llama.cpp is configured)
if(BUILD_WITH_CUDA)
    if(APPLE)
        message(WARNING "CUDA is not available on macOS; disabling BUILD_WITH_CUDA")
        set(BUILD_WITH_CUDA OFF CACHE BOOL "" FORCE)
    else()
        find_package(CUDAToolkit QUIET)
        if(CUDAToolkit_FOUND)
            # Build llama.cpp / ggml with CUDA backend.
            # (This is separate from our own USE_CUDA which is applied to llm-node target later.)
            set(GGML_CUDA ON CACHE BOOL "" FORCE)
        else()
            message(WARNING "CUDA toolkit not found, disabling CUDA support")
            set(BUILD_WITH_CUDA OFF CACHE BOOL "" FORCE)
        endif()
    endif()
endif()

# ggml/llama.cpp の ARM ネイティブ最適化は、実行環境の命令対応を誤検出すると
# ビルドエラーになるため、ポータブル設定に固定する。
set(GGML_NATIVE OFF CACHE BOOL "Disable native cpu flags for portability" FORCE)
set(GGML_CPU_ARM_ARCH "armv8-a" CACHE STRING "Baseline ARM arch" FORCE)
set(GGML_CPU_ALL_VARIANTS OFF CACHE BOOL "Single ARM backend variant" FORCE)
set(GGML_INTERNAL_FP16_VECTOR_ARITHMETIC OFF CACHE BOOL "Force disable FP16 vector arithmetic" FORCE)
set(GGML_INTERNAL_MATMUL_INT8 OFF CACHE BOOL "Force disable i8mm" FORCE)
set(GGML_INTERNAL_DOTPROD OFF CACHE BOOL "Force disable dotprod" FORCE)
set(GGML_INTERNAL_SVE OFF CACHE BOOL "Force disable SVE" FORCE)

# Add third party libraries
set(LLAMA_BUILD_GIT_VERSION OFF CACHE BOOL "" FORCE)
set(GGML_BUILD_GIT_VERSION OFF CACHE BOOL "" FORCE)
# Enable common library for chat template support (minja Jinja parser)
set(LLAMA_BUILD_COMMON ON CACHE BOOL "" FORCE)
set(LLAMA_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_TOOLS OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_TESTS OFF CACHE BOOL "" FORCE)
add_subdirectory(third_party/llama.cpp)

# mtmd requires LLAMA_INSTALL_VERSION which is scoped to llama.cpp subdirectory
# Define it here so mtmd can build correctly when added separately
set(LLAMA_INSTALL_VERSION "0.0.0" CACHE STRING "llama.cpp install version for mtmd" FORCE)
add_subdirectory(third_party/llama.cpp/tools/mtmd EXCLUDE_FROM_ALL)
add_subdirectory(third_party/nlohmann-json)

# whisper.cpp for ASR (Speech-to-Text)
option(BUILD_WITH_WHISPER "Build with whisper.cpp support for ASR" ON)
if(BUILD_WITH_WHISPER)
    if(NOT EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/third_party/whisper.cpp/CMakeLists.txt")
        message(WARNING "third_party/whisper.cpp not found; disabling BUILD_WITH_WHISPER")
        set(BUILD_WITH_WHISPER OFF)
    endif()
endif()
if(BUILD_WITH_WHISPER)
    set(WHISPER_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
    set(WHISPER_BUILD_TESTS OFF CACHE BOOL "" FORCE)
    add_subdirectory(third_party/whisper.cpp)
endif()

# stable-diffusion.cpp for Image Generation
option(BUILD_WITH_SD "Build with stable-diffusion.cpp support for image generation" ON)
if(BUILD_WITH_SD)
    if(NOT EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/third_party/stable-diffusion.cpp/CMakeLists.txt")
        message(WARNING "third_party/stable-diffusion.cpp not found; disabling BUILD_WITH_SD")
        set(BUILD_WITH_SD OFF)
    endif()
endif()
if(BUILD_WITH_SD)
    set(SD_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
    # stable-diffusion.cpp uses its own ggml, configure it to use system ggml if possible
    # For now, let it build its own to avoid version conflicts
    if(APPLE AND BUILD_WITH_METAL AND BUILD_WITH_SD_METAL)
        set(SD_METAL ON CACHE BOOL "" FORCE)
    endif()
    if(BUILD_WITH_CUDA)
        set(SD_CUDA ON CACHE BOOL "" FORCE)
    endif()
    add_subdirectory(third_party/stable-diffusion.cpp)
endif()

# ONNX Runtime for TTS (Text-to-Speech)
option(BUILD_WITH_ONNX "Build with ONNX Runtime support for TTS" OFF)
if(BUILD_WITH_ONNX)
    find_package(onnxruntime QUIET)
    if(onnxruntime_FOUND)
        add_definitions(-DUSE_ONNX_RUNTIME)
        message(STATUS "ONNX Runtime found: ${onnxruntime_VERSION}")
    else()
        message(STATUS "ONNX Runtime not found, TTS support disabled")
        set(BUILD_WITH_ONNX OFF)
    endif()
endif()
include(FetchContent)
FetchContent_Declare(
    spdlog
    GIT_REPOSITORY https://github.com/gabime/spdlog.git
    GIT_TAG v1.14.1
)
FetchContent_MakeAvailable(spdlog)

# Include directories
include_directories(
    ${CMAKE_CURRENT_SOURCE_DIR}/include
    ${CMAKE_CURRENT_SOURCE_DIR}/third_party/cpp-httplib
    ${CMAKE_CURRENT_SOURCE_DIR}/third_party/nlohmann-json/include
    ${CMAKE_CURRENT_SOURCE_DIR}/third_party/llama.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/third_party/llama.cpp/include
    ${CMAKE_CURRENT_SOURCE_DIR}/third_party/llama.cpp/common
    ${CMAKE_CURRENT_SOURCE_DIR}/third_party/llama.cpp/vendor  # T166: minja Jinja2 parser
    ${CMAKE_CURRENT_SOURCE_DIR}/third_party/safetensors-cpp
)

# OpenAI gpt-oss headers
if(BUILD_WITH_GPTOSS)
    if(APPLE AND BUILD_WITH_METAL)
        include_directories(
            ${CMAKE_CURRENT_SOURCE_DIR}/third_party/openai-gpt-oss/gpt_oss/metal/include
            ${CMAKE_CURRENT_SOURCE_DIR}/third_party/openai-gpt-oss/gpt_oss/metal/source/include
        )
    elseif(WIN32)
        include_directories(
            ${CMAKE_CURRENT_SOURCE_DIR}/third_party/openai-gpt-oss/gpt_oss/metal/include
        )
    endif()
endif()

# whisper.cpp include directories
if(BUILD_WITH_WHISPER)
    include_directories(
        ${CMAKE_CURRENT_SOURCE_DIR}/third_party/whisper.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/third_party/whisper.cpp/include
    )
endif()

# stable-diffusion.cpp include directories
if(BUILD_WITH_SD)
    include_directories(
        ${CMAKE_CURRENT_SOURCE_DIR}/third_party/stable-diffusion.cpp
    )
endif()

# Find required packages
find_package(Threads REQUIRED)
find_package(OpenSSL REQUIRED)

# Metal support (macOS)
if(APPLE AND BUILD_WITH_METAL)
    find_library(METAL_FRAMEWORK Metal)
    find_library(METALKIT_FRAMEWORK MetalKit)
    find_library(FOUNDATION_FRAMEWORK Foundation)
    find_library(IOKIT_FRAMEWORK IOKit)
    if(METAL_FRAMEWORK AND METALKIT_FRAMEWORK AND FOUNDATION_FRAMEWORK)
        add_definitions(-DUSE_METAL)
        link_libraries(${METAL_FRAMEWORK} ${METALKIT_FRAMEWORK} ${FOUNDATION_FRAMEWORK})

        if(BUILD_WITH_GPTOSS)
            execute_process(
                COMMAND xcrun -sdk macosx -find metal
                OUTPUT_VARIABLE GPTOSS_METAL_COMPILER
                OUTPUT_STRIP_TRAILING_WHITESPACE
                ERROR_QUIET
                RESULT_VARIABLE GPTOSS_METAL_COMPILER_RESULT
            )
            execute_process(
                COMMAND xcrun -sdk macosx -find metallib
                OUTPUT_VARIABLE GPTOSS_METALLIB
                OUTPUT_STRIP_TRAILING_WHITESPACE
                ERROR_QUIET
                RESULT_VARIABLE GPTOSS_METALLIB_RESULT
            )

            if(NOT GPTOSS_METAL_COMPILER_RESULT EQUAL 0 OR NOT GPTOSS_METALLIB_RESULT EQUAL 0)
                message(WARNING "xcrun metal/metallib not found; disabling BUILD_WITH_GPTOSS. Install Xcode Command Line Tools (xcode-select --install) to enable.")
                set(BUILD_WITH_GPTOSS OFF CACHE BOOL "" FORCE)
            else()
                add_definitions(-DUSE_GPTOSS)
            endif()
        endif()
    else()
        message(WARNING "Metal frameworks not found, disabling Metal support")
        set(BUILD_WITH_METAL OFF)
    endif()
endif()

# DirectML support (Windows)
if(WIN32 AND BUILD_WITH_GPTOSS)
    add_definitions(-DUSE_GPTOSS)
    if(BUILD_WITH_DIRECTML)
        add_definitions(-DUSE_DIRECTML)
    endif()
endif()

# ROCm support
if(BUILD_WITH_ROCM)
    find_package(hip)
    if(hip_FOUND)
        add_definitions(-DUSE_ROCM)
        include_directories(${HIP_INCLUDE_DIRS})
        link_libraries(${HIP_LIBRARIES})
    else()
        message(WARNING "ROCm not found, disabling ROCm support")
        set(BUILD_WITH_ROCM OFF)
    endif()
endif()

# OpenAI gpt-oss Metal library (macOS only)
if(APPLE AND BUILD_WITH_METAL AND BUILD_WITH_GPTOSS)
    set(GPTOSS_METAL_DIR ${CMAKE_CURRENT_SOURCE_DIR}/third_party/openai-gpt-oss/metal)
    if(NOT EXISTS "${GPTOSS_METAL_DIR}/source")
        set(GPTOSS_METAL_DIR ${CMAKE_CURRENT_SOURCE_DIR}/third_party/openai-gpt-oss/gpt_oss/metal)
    endif()
    if(NOT EXISTS "${GPTOSS_METAL_DIR}/source")
        message(FATAL_ERROR "gpt-oss Metal sources not found: ${GPTOSS_METAL_DIR}/source")
    endif()
    set(GPTOSS_METAL_LIB default.metallib)

    add_custom_command(
        OUTPUT  ${CMAKE_CURRENT_BINARY_DIR}/${GPTOSS_METAL_LIB}
        COMMAND ${CMAKE_COMMAND} -E make_directory "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/accumulate.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/accumulate.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/convert.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/convert.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/embeddings.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/embeddings.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/expert_routing_metadata.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/expert_routing_metadata.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/matmul.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/matmul.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/moematmul.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/moematmul.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/gather_and_accumulate.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/gather_and_accumulate.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/random.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/random.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/rmsnorm.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/rmsnorm.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/rope.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/rope.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/sample.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/sample.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/scatter.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/scatter.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/sdpa.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/sdpa.air"
        COMMAND xcrun -sdk macosx metal -g "-I${GPTOSS_METAL_DIR}/include" -g "-I${GPTOSS_METAL_DIR}/source/include" -c "${GPTOSS_METAL_DIR}/source/topk.metal" -o "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/topk.air"
        COMMAND xcrun -sdk macosx metallib
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/accumulate.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/convert.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/embeddings.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/expert_routing_metadata.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/gather_and_accumulate.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/matmul.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/moematmul.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/random.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/rmsnorm.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/rope.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/sample.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/scatter.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/sdpa.air"
                "${CMAKE_CURRENT_BINARY_DIR}/gptoss_metal/topk.air"
                -o "${CMAKE_CURRENT_BINARY_DIR}/${GPTOSS_METAL_LIB}"
        DEPENDS
            "${GPTOSS_METAL_DIR}/source/accumulate.metal"
            "${GPTOSS_METAL_DIR}/source/convert.metal"
            "${GPTOSS_METAL_DIR}/source/embeddings.metal"
            "${GPTOSS_METAL_DIR}/source/expert_routing_metadata.metal"
            "${GPTOSS_METAL_DIR}/source/gather_and_accumulate.metal"
            "${GPTOSS_METAL_DIR}/source/matmul.metal"
            "${GPTOSS_METAL_DIR}/source/moematmul.metal"
            "${GPTOSS_METAL_DIR}/source/random.metal"
            "${GPTOSS_METAL_DIR}/source/rmsnorm.metal"
            "${GPTOSS_METAL_DIR}/source/rope.metal"
            "${GPTOSS_METAL_DIR}/source/sample.metal"
            "${GPTOSS_METAL_DIR}/source/scatter.metal"
            "${GPTOSS_METAL_DIR}/source/sdpa.metal"
            "${GPTOSS_METAL_DIR}/source/topk.metal"
        COMMENT "Compiling gpt-oss Metal kernels"
    )
    add_custom_target(gptoss_build_metallib DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/${GPTOSS_METAL_LIB})

    add_library(gptoss_log OBJECT ${GPTOSS_METAL_DIR}/source/log.c)

    add_library(gptoss_metal_kernels STATIC ${GPTOSS_METAL_DIR}/source/metal.m ${GPTOSS_METAL_DIR}/source/metal-kernels.c)
    target_link_libraries(gptoss_metal_kernels PRIVATE gptoss_log)
    target_include_directories(gptoss_metal_kernels PRIVATE ${GPTOSS_METAL_DIR}/include ${GPTOSS_METAL_DIR}/source/include)
    add_dependencies(gptoss_metal_kernels gptoss_build_metallib)
    if(IOKIT_FRAMEWORK)
        target_link_libraries(gptoss_metal_kernels PRIVATE ${IOKIT_FRAMEWORK})
    endif()

    add_library(gptoss_metal STATIC
        ${GPTOSS_METAL_DIR}/source/model.c
        ${GPTOSS_METAL_DIR}/source/tokenizer.c
        ${GPTOSS_METAL_DIR}/source/context.c
    )
    target_include_directories(gptoss_metal PRIVATE ${GPTOSS_METAL_DIR}/include ${GPTOSS_METAL_DIR}/source/include)
    target_link_libraries(gptoss_metal PRIVATE gptoss_log gptoss_metal_kernels)
endif()

# OpenAI gpt-oss DirectML runtime (Windows, built in-tree)
if(WIN32 AND BUILD_WITH_GPTOSS AND (BUILD_WITH_DIRECTML OR BUILD_WITH_CUDA))
    add_library(gptoss_directml SHARED
        ${CMAKE_CURRENT_SOURCE_DIR}/src/directml/gptoss_directml.cpp
    )
    target_include_directories(gptoss_directml
        PRIVATE
            ${CMAKE_CURRENT_SOURCE_DIR}/third_party/openai-gpt-oss/gpt_oss/metal/include
            ${CMAKE_CURRENT_SOURCE_DIR}/third_party/nlohmann-json/include
    )
    target_link_libraries(gptoss_directml PRIVATE d3d12 dxgi dxguid)
    target_compile_definitions(gptoss_directml PRIVATE GPTOSS_ABI=__declspec(dllexport))
    set_target_properties(gptoss_directml PROPERTIES OUTPUT_NAME "gptoss_directml")

    add_library(nemotron_directml SHARED
        ${CMAKE_CURRENT_SOURCE_DIR}/src/directml/gptoss_directml.cpp
    )
    target_include_directories(nemotron_directml
        PRIVATE
            ${CMAKE_CURRENT_SOURCE_DIR}/third_party/openai-gpt-oss/gpt_oss/metal/include
            ${CMAKE_CURRENT_SOURCE_DIR}/third_party/nlohmann-json/include
    )
    target_link_libraries(nemotron_directml PRIVATE d3d12 dxgi dxguid)
    target_compile_definitions(nemotron_directml PRIVATE GPTOSS_ABI=__declspec(dllexport))
    set_target_properties(nemotron_directml PROPERTIES OUTPUT_NAME "nemotron_directml")
endif()

# Utils library (config etc.)
add_library(utils_lib STATIC
    ${CMAKE_CURRENT_SOURCE_DIR}/src/utils/config.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/utils/logger.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/utils/json_utils.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/utils/utf8.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/utils/system_info.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/utils/request_id.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/utils/cli.cpp
)
target_include_directories(utils_lib PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include)
target_link_libraries(utils_lib PUBLIC spdlog::spdlog_header_only)

# Engine plugin: llama.cpp
set(LLAMA_ENGINE_SOURCES
    ${CMAKE_CURRENT_SOURCE_DIR}/engines/llama_cpp/llama_engine_plugin.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/core/llama_engine.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/core/llama_manager.cpp
)
# Add platform-specific gpu_detector
if(APPLE)
    list(APPEND LLAMA_ENGINE_SOURCES ${CMAKE_CURRENT_SOURCE_DIR}/src/system/gpu_detector.mm)
else()
    list(APPEND LLAMA_ENGINE_SOURCES ${CMAKE_CURRENT_SOURCE_DIR}/src/system/gpu_detector.cpp)
endif()
add_library(llm_engine_llama_cpp SHARED ${LLAMA_ENGINE_SOURCES})
target_include_directories(llm_engine_llama_cpp
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/include
        ${CMAKE_CURRENT_SOURCE_DIR}/src
        ${CMAKE_CURRENT_SOURCE_DIR}/third_party/llama.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/third_party/llama.cpp/include
        ${CMAKE_CURRENT_SOURCE_DIR}/third_party/llama.cpp/common
)
target_link_libraries(llm_engine_llama_cpp
    PRIVATE
        llama
        spdlog::spdlog_header_only
)
# Link Metal framework on Apple for gpu_detector
if(APPLE)
    target_link_libraries(llm_engine_llama_cpp PRIVATE "-framework Metal" "-framework Foundation")
endif()
set_target_properties(llm_engine_llama_cpp PROPERTIES OUTPUT_NAME "llm_engine_llama_cpp")

# Engine plugin: gpt-oss
if(APPLE AND BUILD_WITH_METAL AND BUILD_WITH_GPTOSS)
    add_library(llm_engine_gptoss SHARED
        ${CMAKE_CURRENT_SOURCE_DIR}/engines/gptoss/gptoss_engine_plugin.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/src/core/gptoss_engine.cpp
    )
    target_include_directories(llm_engine_gptoss
        PRIVATE
            ${CMAKE_CURRENT_SOURCE_DIR}/include
            ${CMAKE_CURRENT_SOURCE_DIR}/src
            ${GPTOSS_METAL_DIR}/include
            ${GPTOSS_METAL_DIR}/source/include
    )
    target_link_libraries(llm_engine_gptoss
        PRIVATE
            gptoss_metal
            spdlog::spdlog_header_only
    )
    add_dependencies(llm_engine_gptoss gptoss_build_metallib)
    target_link_options(llm_engine_gptoss PRIVATE
        LINKER:-sectcreate,__METAL,__shaders,${CMAKE_CURRENT_BINARY_DIR}/${GPTOSS_METAL_LIB}
    )
    set_target_properties(llm_engine_gptoss PROPERTIES OUTPUT_NAME "llm_engine_gptoss")
elseif(WIN32 AND BUILD_WITH_GPTOSS)
    add_library(llm_engine_gptoss SHARED
        ${CMAKE_CURRENT_SOURCE_DIR}/engines/gptoss/gptoss_engine_plugin.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/src/core/gptoss_engine.cpp
    )
    target_include_directories(llm_engine_gptoss
        PRIVATE
            ${CMAKE_CURRENT_SOURCE_DIR}/include
            ${CMAKE_CURRENT_SOURCE_DIR}/src
    )
    target_link_libraries(llm_engine_gptoss
        PRIVATE
            spdlog::spdlog_header_only
    )
    if(BUILD_WITH_CUDA)
        target_compile_definitions(llm_engine_gptoss PRIVATE USE_CUDA)
    endif()
    set_target_properties(llm_engine_gptoss PROPERTIES OUTPUT_NAME "llm_engine_gptoss")
endif()

# Engine plugin: nemotron (DirectML on Windows)
if(WIN32 AND BUILD_WITH_GPTOSS AND BUILD_WITH_DIRECTML)
    add_library(llm_engine_nemotron SHARED
        ${CMAKE_CURRENT_SOURCE_DIR}/engines/nemotron/nemotron_engine_plugin.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/src/core/nemotron_engine.cpp
    )
    target_include_directories(llm_engine_nemotron
        PRIVATE
            ${CMAKE_CURRENT_SOURCE_DIR}/include
            ${CMAKE_CURRENT_SOURCE_DIR}/src
    )
    target_link_libraries(llm_engine_nemotron
        PRIVATE
            spdlog::spdlog_header_only
    )
    if(BUILD_WITH_CUDA)
        target_compile_definitions(llm_engine_nemotron PRIVATE USE_CUDA)
    endif()
    set_target_properties(llm_engine_nemotron PROPERTIES OUTPUT_NAME "llm_engine_nemotron")
endif()

# Collect source files
file(GLOB_RECURSE SOURCES
    ${CMAKE_CURRENT_SOURCE_DIR}/src/*.cpp
)
# Exclude DirectML files (Windows-only, built separately)
list(FILTER SOURCES EXCLUDE REGEX ".*/directml/.*")

# Include Objective-C++ files on Apple platforms
if(APPLE)
    file(GLOB_RECURSE OBJCXX_SOURCES
        ${CMAKE_CURRENT_SOURCE_DIR}/src/*.mm
    )
    list(APPEND SOURCES ${OBJCXX_SOURCES})
endif()

# Create executable
add_executable(llm-node ${SOURCES})

# Link libraries
target_link_libraries(llm-node PRIVATE
    llama
    common
    mtmd
    utils_lib
    ${CMAKE_THREAD_LIBS_INIT}
    OpenSSL::SSL
    OpenSSL::Crypto
)

target_compile_definitions(llm-node PRIVATE CPPHTTPLIB_OPENSSL_SUPPORT)

# CUDA support (llm-node)
if(BUILD_WITH_CUDA AND CUDAToolkit_FOUND)
    target_compile_definitions(llm-node PRIVATE USE_CUDA)
    target_link_libraries(llm-node PRIVATE CUDA::cudart)
    if(TARGET CUDA::nvml)
        target_link_libraries(llm-node PRIVATE CUDA::nvml)
    else()
        find_library(NVML_LIBRARY nvidia-ml)
        if(NVML_LIBRARY)
            target_link_libraries(llm-node PRIVATE ${NVML_LIBRARY})
        endif()
    endif()
endif()

# gpt-oss Metal backend (macOS)
if(APPLE AND BUILD_WITH_METAL AND BUILD_WITH_GPTOSS)
    target_link_libraries(llm-node PRIVATE gptoss_metal)
    add_dependencies(llm-node gptoss_build_metallib)
    target_link_options(llm-node PRIVATE
        LINKER:-sectcreate,__METAL,__shaders,${CMAKE_CURRENT_BINARY_DIR}/${GPTOSS_METAL_LIB}
    )
endif()

# whisper.cpp support
if(BUILD_WITH_WHISPER)
    target_link_libraries(llm-node PRIVATE whisper)
    target_compile_definitions(llm-node PRIVATE USE_WHISPER)
endif()

# stable-diffusion.cpp support
if(BUILD_WITH_SD)
    target_link_libraries(llm-node PRIVATE stable-diffusion)
    target_compile_definitions(llm-node PRIVATE USE_SD)
endif()

# ONNX Runtime support
if(BUILD_WITH_ONNX)
    target_link_libraries(llm-node PRIVATE onnxruntime::onnxruntime)
endif()

# Static linking if requested
if(STATIC_BUILD)
    if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
        set_target_properties(llm-node PROPERTIES
            LINK_FLAGS "-static"
        )
    endif()
endif()

# Installation
install(TARGETS llm-node
    RUNTIME DESTINATION bin
)

# Tests
if(BUILD_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()

# Packaging (DEB/RPM via CPack) - only if LICENSE file exists
set(CPACK_PACKAGE_NAME "llm-node")
set(CPACK_PACKAGE_VENDOR "llm-router")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "LLM inference node with llama.cpp")
set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
set(CPACK_PACKAGE_CONTACT "oss@llm-router.local")
set(LICENSE_FILE "${CMAKE_CURRENT_SOURCE_DIR}/../LICENSE")
if(EXISTS "${LICENSE_FILE}")
    set(CPACK_RESOURCE_FILE_LICENSE "${LICENSE_FILE}")
    set(CPACK_GENERATOR "DEB;RPM")
    set(CPACK_DEBIAN_PACKAGE_DEPENDS "libssl3")
    set(CPACK_RPM_PACKAGE_REQUIRES "openssl-libs")
    include(CPack)
else()
    message(STATUS "LICENSE file not found, skipping CPack configuration")
endif()

# Print configuration summary
message(STATUS "")
message(STATUS "llm-node configuration summary:")
message(STATUS "  Build type:        ${CMAKE_BUILD_TYPE}")
message(STATUS "  C++ standard:      ${CMAKE_CXX_STANDARD}")
message(STATUS "  Compiler:          ${CMAKE_CXX_COMPILER_ID}")
message(STATUS "  CUDA support:      ${BUILD_WITH_CUDA}")
message(STATUS "  Metal support:     ${BUILD_WITH_METAL}")
message(STATUS "  ROCm support:      ${BUILD_WITH_ROCM}")
message(STATUS "  Whisper support:   ${BUILD_WITH_WHISPER}")
message(STATUS "  SD support:        ${BUILD_WITH_SD}")
message(STATUS "  ONNX support:      ${BUILD_WITH_ONNX}")
message(STATUS "  Static build:      ${STATIC_BUILD}")
message(STATUS "  Build tests:       ${BUILD_TESTS}")
message(STATUS "")
