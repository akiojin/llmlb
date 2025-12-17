cmake_minimum_required(VERSION 3.20)
project(onnx-runtime-demo LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

find_package(onnxruntime REQUIRED)

add_executable(onnx_poc main.cpp)
target_link_libraries(onnx_poc PRIVATE onnxruntime::onnxruntime)

