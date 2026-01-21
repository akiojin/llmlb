cmake_minimum_required(VERSION 3.20)
project(allm VERSION 1.0.0 LANGUAGES CXX C)

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
    # Ensure standard C++ exception unwinding on MSVC
    add_compile_options($<$<COMPILE_LANGUAGE:CXX>:/EHsc>)
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
            # (This is separate from our own USE_CUDA which is applied to allm target later.)
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
)

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
    else()
        message(WARNING "Metal frameworks not found, disabling Metal support")
        set(BUILD_WITH_METAL OFF)
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

# CLI library (SPEC-58378000: CLI commands)
add_library(cli_lib STATIC
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/cli_client.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/repl_session.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/progress_renderer.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/ollama_compat.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/commands/serve.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/commands/run.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/commands/pull.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/commands/list.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/commands/show.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/commands/rm.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/commands/stop.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/commands/ps.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/commands/router_endpoints.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/commands/router_models.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/cli/commands/router_status.cpp
)
target_include_directories(cli_lib PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include)
target_link_libraries(cli_lib PUBLIC
    utils_lib
    spdlog::spdlog_header_only
    OpenSSL::SSL
    OpenSSL::Crypto
)
target_compile_definitions(cli_lib PRIVATE CPPHTTPLIB_OPENSSL_SUPPORT)

# ============================================================================
# safetensors.cpp integration (SPEC-69549000)
# ============================================================================
option(BUILD_WITH_SAFETENSORS "Build with safetensors.cpp support" ON)
if(BUILD_WITH_SAFETENSORS)
    if(NOT EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/third_party/safetensors.cpp/CMakeLists.txt")
        message(WARNING "third_party/safetensors.cpp not found; disabling BUILD_WITH_SAFETENSORS")
        set(BUILD_WITH_SAFETENSORS OFF)
    endif()
endif()

if(BUILD_WITH_SAFETENSORS)
    # Configure safetensors.cpp options
    set(STCPP_METAL ${BUILD_WITH_METAL} CACHE BOOL "" FORCE)
    set(STCPP_CUDA ${BUILD_WITH_CUDA} CACHE BOOL "" FORCE)
    set(STCPP_ROCM ${BUILD_WITH_ROCM} CACHE BOOL "" FORCE)
    set(STCPP_BUILD_TESTS OFF CACHE BOOL "" FORCE)
    set(STCPP_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
    add_subdirectory(third_party/safetensors.cpp)

    # safetensors engine sources are compiled into the allm binary
endif()

# Collect source files
file(GLOB_RECURSE SOURCES
    ${CMAKE_CURRENT_SOURCE_DIR}/src/*.cpp
)
# Include safetensors engine in the main binary for TextManager
if(BUILD_WITH_SAFETENSORS)
    list(APPEND SOURCES ${CMAKE_CURRENT_SOURCE_DIR}/engines/safetensors/safetensors_engine.cpp)
endif()
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
add_executable(allm ${SOURCES})

if(BUILD_WITH_SAFETENSORS)
    target_include_directories(allm PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/engines/safetensors
        ${CMAKE_CURRENT_SOURCE_DIR}/third_party/safetensors.cpp/include
        ${CMAKE_CURRENT_SOURCE_DIR}/third_party/nlohmann-json/include
    )
    target_link_libraries(allm PRIVATE safetensors_cpp)
    target_compile_definitions(allm PRIVATE ALLM_WITH_SAFETENSORS)
endif()

# Link libraries
target_link_libraries(allm PRIVATE
    llama
    common
    mtmd
    utils_lib
    ${CMAKE_THREAD_LIBS_INIT}
    OpenSSL::SSL
    OpenSSL::Crypto
)

target_compile_definitions(allm PRIVATE CPPHTTPLIB_OPENSSL_SUPPORT)

# CUDA support (allm)
if(BUILD_WITH_CUDA AND CUDAToolkit_FOUND)
    target_compile_definitions(allm PRIVATE USE_CUDA)
    target_link_libraries(allm PRIVATE CUDA::cudart)
    if(TARGET CUDA::nvml)
        target_link_libraries(allm PRIVATE CUDA::nvml)
    else()
        find_library(NVML_LIBRARY nvidia-ml)
        if(NVML_LIBRARY)
            target_link_libraries(allm PRIVATE ${NVML_LIBRARY})
        endif()
    endif()
endif()

# whisper.cpp support
if(BUILD_WITH_WHISPER)
    target_link_libraries(allm PRIVATE whisper)
    target_compile_definitions(allm PRIVATE USE_WHISPER)
endif()

# stable-diffusion.cpp support
if(BUILD_WITH_SD)
    target_link_libraries(allm PRIVATE stable-diffusion)
    target_compile_definitions(allm PRIVATE USE_SD)
endif()

# ONNX Runtime support
if(BUILD_WITH_ONNX)
    target_link_libraries(allm PRIVATE onnxruntime::onnxruntime)
endif()

# Static linking if requested
if(STATIC_BUILD)
    if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
        set_target_properties(allm PROPERTIES
            LINK_FLAGS "-static"
        )
    endif()
endif()

# Installation
install(TARGETS allm
    RUNTIME DESTINATION bin
)

# Tests
if(BUILD_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()

# Packaging (DEB/RPM via CPack) - only if LICENSE file exists
set(CPACK_PACKAGE_NAME "allm")
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
message(STATUS "allm configuration summary:")
message(STATUS "  Build type:        ${CMAKE_BUILD_TYPE}")
message(STATUS "  C++ standard:      ${CMAKE_CXX_STANDARD}")
message(STATUS "  Compiler:          ${CMAKE_CXX_COMPILER_ID}")
message(STATUS "  CUDA support:      ${BUILD_WITH_CUDA}")
message(STATUS "  Metal support:     ${BUILD_WITH_METAL}")
message(STATUS "  ROCm support:      ${BUILD_WITH_ROCM}")
message(STATUS "  Whisper support:   ${BUILD_WITH_WHISPER}")
message(STATUS "  SD support:        ${BUILD_WITH_SD}")
message(STATUS "  safetensors.cpp:   ${BUILD_WITH_SAFETENSORS}")
message(STATUS "  ONNX support:      ${BUILD_WITH_ONNX}")
message(STATUS "  Static build:      ${STATIC_BUILD}")
message(STATUS "  Build tests:       ${BUILD_TESTS}")
message(STATUS "")
