cmake_minimum_required(VERSION 3.18)
project(nemotron-cuda-poc LANGUAGES CXX CUDA)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CUDA_STANDARD 17)
set(CMAKE_CUDA_STANDARD_REQUIRED ON)

# CUDA Toolkit
find_package(CUDAToolkit REQUIRED)

# Architecture detection (SM 70+)
if(NOT DEFINED CMAKE_CUDA_ARCHITECTURES)
    set(CMAKE_CUDA_ARCHITECTURES 70 75 80 86 89 90)
endif()

# Source files
set(SOURCES
    src/main.cpp
    src/model_config.cpp
    src/safetensors_loader.cpp
    src/tokenizer.cpp
    src/inference.cpp
    src/cuda_memory.cu
    src/transformer.cu
    src/kernels/rms_norm.cu
    src/kernels/silu.cu
    src/kernels/softmax.cu
    src/kernels/embedding.cu
    src/kernels/attention.cu
)

# Executable
add_executable(nemotron-cuda-poc ${SOURCES})

# Include directories
target_include_directories(nemotron-cuda-poc PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)

# Link libraries
target_link_libraries(nemotron-cuda-poc PRIVATE
    CUDA::cudart
    CUDA::cublas
)

# Compiler flags
if(MSVC)
    target_compile_options(nemotron-cuda-poc PRIVATE
        $<$<COMPILE_LANGUAGE:CXX>:/W4 /utf-8>
        $<$<COMPILE_LANGUAGE:CUDA>:--expt-relaxed-constexpr -Xcompiler=/EHsc -Xcompiler=/utf-8>
    )
else()
    target_compile_options(nemotron-cuda-poc PRIVATE
        $<$<COMPILE_LANGUAGE:CXX>:-Wall -Wextra>
        $<$<COMPILE_LANGUAGE:CUDA>:--expt-relaxed-constexpr>
    )
endif()

# Windows-specific settings
if(WIN32)
    target_compile_definitions(nemotron-cuda-poc PRIVATE
        _CRT_SECURE_NO_WARNINGS
        NOMINMAX
    )
endif()

# Debug/Release configurations
if(MSVC)
    set(CMAKE_CXX_FLAGS_DEBUG "/Zi /Od")
    set(CMAKE_CXX_FLAGS_RELEASE "/O2 /DNDEBUG")
    set(CMAKE_CUDA_FLAGS_RELEASE "-O3 -DNDEBUG")
else()
    set(CMAKE_CXX_FLAGS_DEBUG "-g -O0")
    set(CMAKE_CXX_FLAGS_RELEASE "-O3 -DNDEBUG")
    set(CMAKE_CUDA_FLAGS_DEBUG "-g -G -O0")
    set(CMAKE_CUDA_FLAGS_RELEASE "-O3 -DNDEBUG")
endif()
