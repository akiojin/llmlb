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
