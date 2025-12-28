set(CONTRACT_TEST_SOURCES
    openai_api_test.cpp
    router_api_test.cpp
    ../../src/api/http_server.cpp
    ../../src/api/openai_endpoints.cpp
    ../../src/api/node_endpoints.cpp
    ../../src/models/model_registry.cpp
    ../../src/models/model_sync.cpp
    ../../src/models/model_downloader.cpp
    ../../src/models/model_storage.cpp
    ../../src/metrics/prometheus_exporter.cpp
    ../../src/core/engine_registry.cpp
    ../../src/core/engine_host.cpp
    ../../src/core/llama_engine.cpp
    ../../src/core/gptoss_engine.cpp
    ../../src/core/nemotron_engine.cpp
    ../../src/core/inference_engine.cpp
    ../../src/core/llama_manager.cpp
    ../../src/api/router_client.cpp
    ../../src/runtime/state.cpp
)

add_executable(llm-node-contract-tests
    ${CONTRACT_TEST_SOURCES}
)

target_link_libraries(llm-node-contract-tests
    PRIVATE
        test_common
        llama
        common
        nlohmann_json::nlohmann_json
        utils_lib
        OpenSSL::SSL
        OpenSSL::Crypto
)

target_include_directories(llm-node-contract-tests
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/../../include
        ${CMAKE_CURRENT_SOURCE_DIR}/../../src
)

# Link Metal frameworks / gpt-oss metal library on macOS
if(APPLE)
    find_library(METAL_FRAMEWORK Metal)
    find_library(FOUNDATION_FRAMEWORK Foundation)
    if(METAL_FRAMEWORK AND FOUNDATION_FRAMEWORK)
        target_link_libraries(llm-node-contract-tests PRIVATE ${METAL_FRAMEWORK} ${FOUNDATION_FRAMEWORK})
        target_compile_definitions(llm-node-contract-tests PRIVATE USE_METAL=1)
    endif()

    if(BUILD_WITH_GPTOSS)
        target_link_libraries(llm-node-contract-tests PRIVATE gptoss_metal)
        add_dependencies(llm-node-contract-tests gptoss_build_metallib)
        target_link_options(llm-node-contract-tests PRIVATE
            LINKER:-sectcreate,__METAL,__shaders,${CMAKE_BINARY_DIR}/${GPTOSS_METAL_LIB}
        )
    endif()
endif()

add_test(
    NAME llm-node-contract-tests
    COMMAND llm-node-contract-tests
)
