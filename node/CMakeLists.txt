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

# Compiler flags
if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
    add_compile_options(-Wall -Wextra -Wpedantic)
    if(CMAKE_BUILD_TYPE STREQUAL "Debug")
        add_compile_options(-g -O0)
    else()
        add_compile_options(-O3 -march=native)
    endif()
endif()

# Options
option(BUILD_TESTS "Build tests" ON)
option(BUILD_WITH_CUDA "Build with CUDA support" OFF)
if(APPLE)
    option(BUILD_WITH_METAL "Build with Metal support (macOS)" ON)
else()
    option(BUILD_WITH_METAL "Build with Metal support (macOS)" OFF)
endif()
option(BUILD_WITH_ROCM "Build with ROCm support" OFF)
option(STATIC_BUILD "Build statically linked executable" OFF)

# Add third party libraries
add_subdirectory(third_party/nlohmann-json)

# whisper.cpp for ASR (Speech-to-Text)
option(BUILD_WITH_WHISPER "Build with whisper.cpp support for ASR" ON)
if(BUILD_WITH_WHISPER)
    set(WHISPER_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
    set(WHISPER_BUILD_TESTS OFF CACHE BOOL "" FORCE)
    add_subdirectory(third_party/whisper.cpp)
endif()

# stable-diffusion.cpp for Image Generation
# Default OFF: stable-diffusion.cpp is a submodule and should not be required for a basic build.
option(BUILD_WITH_SD "Build with stable-diffusion.cpp support for image generation" OFF)
if(BUILD_WITH_SD)
    set(SD_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
    # stable-diffusion.cpp uses its own ggml, configure it to use system ggml if possible
    # For now, let it build its own to avoid version conflicts
    if(APPLE AND BUILD_WITH_METAL)
        set(SD_METAL ON CACHE BOOL "" FORCE)
    endif()
    if(BUILD_WITH_CUDA)
        set(SD_CUDA ON CACHE BOOL "" FORCE)
    endif()
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/third_party/stable-diffusion.cpp/CMakeLists.txt")
        add_subdirectory(third_party/stable-diffusion.cpp)
    else()
        message(FATAL_ERROR "stable-diffusion.cpp submodule is not initialized. Run: git submodule update --init --recursive")
    endif()
endif()

# ONNX Runtime (LLM/TTS, generic inference)
find_package(onnxruntime REQUIRED)
add_definitions(-DUSE_ONNX_RUNTIME)
message(STATUS "ONNX Runtime found")
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

# CUDA support
if(BUILD_WITH_CUDA)
    find_package(CUDAToolkit)
    if(CUDAToolkit_FOUND)
        add_definitions(-DUSE_CUDA)
        include_directories(${CUDAToolkit_INCLUDE_DIRS})
        link_libraries(${CUDAToolkit_LIBRARIES})
    else()
        message(WARNING "CUDA toolkit not found, disabling CUDA support")
        set(BUILD_WITH_CUDA OFF)
    endif()
endif()

# Metal support (macOS)
if(APPLE AND BUILD_WITH_METAL)
    find_library(METAL_FRAMEWORK Metal)
    find_library(METALKIT_FRAMEWORK MetalKit)
    find_library(FOUNDATION_FRAMEWORK Foundation)
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
    ${CMAKE_CURRENT_SOURCE_DIR}/src/utils/system_info.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/utils/request_id.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/utils/cli.cpp
)
target_include_directories(utils_lib PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include)
target_link_libraries(utils_lib PUBLIC spdlog::spdlog_header_only)

# Collect source files
file(GLOB_RECURSE SOURCES
    ${CMAKE_CURRENT_SOURCE_DIR}/src/*.cpp
)
# Avoid building SD sources unless explicitly enabled (submodule may not be present).
if(NOT BUILD_WITH_SD)
    list(REMOVE_ITEM SOURCES ${CMAKE_CURRENT_SOURCE_DIR}/src/core/sd_manager.cpp)
endif()
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
target_link_libraries(llm-node
    utils_lib
    ${CMAKE_THREAD_LIBS_INIT}
    OpenSSL::SSL
    OpenSSL::Crypto
    onnxruntime::onnxruntime
)

# whisper.cpp support
if(BUILD_WITH_WHISPER)
    target_link_libraries(llm-node whisper)
    target_compile_definitions(llm-node PRIVATE USE_WHISPER)
endif()

# stable-diffusion.cpp support
if(BUILD_WITH_SD)
    target_link_libraries(llm-node stable-diffusion)
    target_compile_definitions(llm-node PRIVATE USE_SD)
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
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "LLM inference node with ONNX Runtime")
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
message(STATUS "  ONNX support:      ON")
message(STATUS "  Static build:      ${STATIC_BUILD}")
message(STATUS "  Build tests:       ${BUILD_TESTS}")
message(STATUS "")
