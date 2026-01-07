cmake_minimum_required(VERSION 3.18)

# Main CLI example
add_executable(stcpp_main main.cpp)
target_link_libraries(stcpp_main
    PRIVATE
        safetensors_cpp
)
target_include_directories(stcpp_main
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/../include
)

# Set output name
set_target_properties(stcpp_main PROPERTIES OUTPUT_NAME "safetensors-cli")

# Benchmark tool
add_executable(stcpp_benchmark benchmark.cpp)
target_link_libraries(stcpp_benchmark
    PRIVATE
        safetensors_cpp
)
target_include_directories(stcpp_benchmark
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/../include
)
set_target_properties(stcpp_benchmark PROPERTIES OUTPUT_NAME "benchmark")

# Pass GPU backend defines
if(STCPP_METAL)
    target_compile_definitions(stcpp_benchmark PRIVATE STCPP_USE_METAL)
endif()
if(STCPP_CUDA)
    target_compile_definitions(stcpp_benchmark PRIVATE STCPP_USE_CUDA)
endif()
if(STCPP_ROCM)
    target_compile_definitions(stcpp_benchmark PRIVATE STCPP_USE_ROCM)
endif()
if(STCPP_VULKAN)
    target_compile_definitions(stcpp_benchmark PRIVATE STCPP_USE_VULKAN)
endif()
