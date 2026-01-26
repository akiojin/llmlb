cmake_minimum_required(VERSION 3.18)
project(safetensors_cpp VERSION 0.1.0 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

# Detect standalone vs integrated build
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    set(STCPP_STANDALONE ON)
    message(STATUS "safetensors.cpp: Standalone build")
else()
    set(STCPP_STANDALONE OFF)
    message(STATUS "safetensors.cpp: Integrated build")
endif()

# Options
option(STCPP_METAL  "Build with Metal support (macOS)" OFF)
option(STCPP_CUDA   "Build with CUDA support" OFF)
option(STCPP_ROCM   "Build with ROCm/HIP support" OFF)
option(STCPP_VULKAN "Build with Vulkan support" OFF)
option(STCPP_BUILD_TESTS    "Build tests" ${STCPP_STANDALONE})
option(STCPP_BUILD_EXAMPLES "Build examples" ${STCPP_STANDALONE})

# Auto-detect Metal on macOS
if(APPLE AND NOT DEFINED STCPP_METAL)
    set(STCPP_METAL ON)
endif()

# ggml dependency
if(STCPP_STANDALONE)
    # Standalone: use ggml submodule
    if(EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/ggml/CMakeLists.txt")
        # Configure ggml options
        set(GGML_METAL ${STCPP_METAL} CACHE BOOL "" FORCE)
        set(GGML_CUDA ${STCPP_CUDA} CACHE BOOL "" FORCE)
        set(GGML_HIP ${STCPP_ROCM} CACHE BOOL "" FORCE)
        set(GGML_VULKAN ${STCPP_VULKAN} CACHE BOOL "" FORCE)
        set(GGML_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
        set(GGML_BUILD_TESTS OFF CACHE BOOL "" FORCE)
        add_subdirectory(ggml)
        set(STCPP_GGML_TARGET ggml)
    else()
        message(FATAL_ERROR "ggml submodule not found. Run: git submodule update --init")
    endif()
else()
    # Integrated: use parent project's ggml (from llama.cpp)
    if(TARGET ggml)
        set(STCPP_GGML_TARGET ggml)
        message(STATUS "safetensors.cpp: Using parent project's ggml")
    else()
        message(FATAL_ERROR "ggml target not found in parent project")
    endif()
endif()

# nlohmann/json dependency (header-only library)
if(TARGET nlohmann_json::nlohmann_json)
    # Integrated build: use parent project's nlohmann_json
    message(STATUS "safetensors.cpp: Using parent project's nlohmann_json")
else()
    # Standalone build or nlohmann_json not available: fetch it
    include(FetchContent)
    FetchContent_Declare(
        nlohmann_json
        GIT_REPOSITORY https://github.com/nlohmann/json.git
        GIT_TAG v3.11.3
    )
    FetchContent_MakeAvailable(nlohmann_json)
endif()

# Include directories
include_directories(
    ${CMAKE_CURRENT_SOURCE_DIR}/include
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

# Collect source files
file(GLOB_RECURSE STCPP_SOURCES
    ${CMAKE_CURRENT_SOURCE_DIR}/src/*.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/src/*.c
)

# Main library
add_library(safetensors_cpp STATIC ${STCPP_SOURCES})
target_include_directories(safetensors_cpp PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)
target_link_libraries(safetensors_cpp
    PUBLIC
        ${STCPP_GGML_TARGET}
        nlohmann_json::nlohmann_json
)

# Enable PIC for linking into shared libraries (required on Linux)
set_target_properties(safetensors_cpp PROPERTIES POSITION_INDEPENDENT_CODE ON)

# Compiler definitions based on backend
if(STCPP_METAL)
    target_compile_definitions(safetensors_cpp PUBLIC STCPP_USE_METAL)
    find_library(METAL_FRAMEWORK Metal)
    find_library(FOUNDATION_FRAMEWORK Foundation)
    if(METAL_FRAMEWORK AND FOUNDATION_FRAMEWORK)
        target_link_libraries(safetensors_cpp PUBLIC
            ${METAL_FRAMEWORK}
            ${FOUNDATION_FRAMEWORK}
        )
    endif()
endif()

if(STCPP_CUDA)
    target_compile_definitions(safetensors_cpp PUBLIC STCPP_USE_CUDA)
endif()

if(STCPP_ROCM)
    target_compile_definitions(safetensors_cpp PUBLIC STCPP_USE_ROCM)
endif()

if(STCPP_VULKAN)
    target_compile_definitions(safetensors_cpp PUBLIC STCPP_USE_VULKAN)
endif()

# Alias for easier usage
add_library(safetensors::cpp ALIAS safetensors_cpp)

# Examples
if(STCPP_BUILD_EXAMPLES)
    add_subdirectory(examples)
endif()

# Tests
if(STCPP_BUILD_TESTS)
    enable_testing()
    add_subdirectory(tests)
endif()

# Installation (for standalone builds)
if(STCPP_STANDALONE)
    install(TARGETS safetensors_cpp
        ARCHIVE DESTINATION lib
        LIBRARY DESTINATION lib
    )
    install(DIRECTORY include/ DESTINATION include)
endif()

# Print configuration
message(STATUS "")
message(STATUS "safetensors.cpp configuration:")
message(STATUS "  Version:      ${PROJECT_VERSION}")
message(STATUS "  Build type:   ${CMAKE_BUILD_TYPE}")
message(STATUS "  Metal:        ${STCPP_METAL}")
message(STATUS "  CUDA:         ${STCPP_CUDA}")
message(STATUS "  ROCm:         ${STCPP_ROCM}")
message(STATUS "  Vulkan:       ${STCPP_VULKAN}")
message(STATUS "  Tests:        ${STCPP_BUILD_TESTS}")
message(STATUS "  Examples:     ${STCPP_BUILD_EXAMPLES}")
message(STATUS "")
