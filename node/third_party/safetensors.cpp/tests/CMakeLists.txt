cmake_minimum_required(VERSION 3.18)

# Fetch Google Test
include(FetchContent)
FetchContent_Declare(
    googletest
    URL https://github.com/google/googletest/archive/refs/tags/v1.15.2.zip
)
# Prevent overriding parent project's compiler/linker settings on Windows
set(gtest_force_shared_crt ON CACHE BOOL "" FORCE)
FetchContent_MakeAvailable(googletest)

# Enable testing
enable_testing()

# Contract tests
add_executable(stcpp_contract_tests
    contract/init_test.cpp
    contract/version_test.cpp
    contract/params_test.cpp
    contract/model_test.cpp
    contract/tokenizer_test.cpp
)
target_link_libraries(stcpp_contract_tests
    PRIVATE
        safetensors_cpp
        GTest::gtest_main
)
target_include_directories(stcpp_contract_tests
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/../include
)

# Unit tests
add_executable(stcpp_unit_tests
    unit/placeholder_test.cpp
    unit/safetensors_parser_test.cpp
    unit/tensor_conversion_test.cpp
    unit/split_loader_test.cpp
    unit/tokenizer_json_test.cpp
    unit/tokenization_test.cpp
    unit/chat_template_test.cpp
    unit/model_load_test.cpp
    unit/generation_test.cpp
    unit/cancel_test.cpp
    unit/kv_cache_test.cpp
    unit/prompt_cache_test.cpp
    unit/batch_test.cpp
    unit/embeddings_test.cpp
    unit/lora_test.cpp
    unit/rope_scaling_test.cpp
    unit/sliding_window_attention_test.cpp
    unit/gqa_mqa_test.cpp
    unit/mamba_ssm_test.cpp
    unit/multi_gpu_test.cpp
)
target_link_libraries(stcpp_unit_tests
    PRIVATE
        safetensors_cpp
        GTest::gtest_main
)
target_include_directories(stcpp_unit_tests
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/../include
        ${CMAKE_CURRENT_SOURCE_DIR}/../src
)

# Integration/E2E tests
add_executable(stcpp_integration_tests
    integration/e2e_test.cpp
)
target_link_libraries(stcpp_integration_tests
    PRIVATE
        safetensors_cpp
        GTest::gtest_main
)
target_include_directories(stcpp_integration_tests
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/../include
        ${CMAKE_CURRENT_SOURCE_DIR}/../src
)

# Register tests with CTest
include(GoogleTest)
gtest_discover_tests(stcpp_contract_tests)
gtest_discover_tests(stcpp_unit_tests)
gtest_discover_tests(stcpp_integration_tests)
