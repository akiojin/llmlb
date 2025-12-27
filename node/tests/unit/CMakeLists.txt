set(UNIT_TEST_SOURCES
    gpu_detector_test.cpp
    router_client_test.cpp
    model_sync_test.cpp
    model_downloader_test.cpp
    model_storage_test.cpp
    inference_engine_test.cpp
    engine_registry_test.cpp
    nemotron_engine_test.cpp
    model_converter_test.cpp
    model_resolver_test.cpp
    llama_manager_test.cpp
    model_pool_test.cpp
    model_loader_test.cpp
    whisper_manager_test.cpp
    onnx_tts_manager_test.cpp
    sd_manager_test.cpp
    utils_config_test.cpp
    utils_misc_test.cpp
    utils_utf8_test.cpp
    request_guard_test.cpp
    cli_test.cpp
    ../../src/utils/cli.cpp
    ../../src/api/router_client.cpp
    ../../src/models/model_sync.cpp
    ../../src/models/model_downloader.cpp
    ../../src/models/model_storage.cpp
    ../../src/models/model_converter.cpp
    ../../src/models/model_resolver.cpp
    ../../src/core/llama_manager.cpp
    ../../src/core/model_pool.cpp
    ../../src/core/engine_registry.cpp
    ../../src/core/llama_engine.cpp
    ../../src/core/gptoss_engine.cpp
    ../../src/core/nemotron_engine.cpp
    ../../src/core/inference_engine.cpp
    ../../src/core/whisper_manager.cpp
    ../../src/core/onnx_tts_manager.cpp
    ../../src/core/sd_manager.cpp
    ../../src/runtime/state.cpp
)

# Use platform-specific gpu_detector source
if(APPLE)
    list(APPEND UNIT_TEST_SOURCES ../../src/system/gpu_detector.mm)
else()
    list(APPEND UNIT_TEST_SOURCES ../../src/system/gpu_detector.cpp)
endif()

add_executable(llm-node-unit-tests
    ${UNIT_TEST_SOURCES}
)

target_link_libraries(llm-node-unit-tests
    PRIVATE
        test_common
        llama
        whisper
        stable-diffusion
        common
        nlohmann_json::nlohmann_json
        utils_lib
        OpenSSL::SSL
        OpenSSL::Crypto
)

# Link Metal frameworks on macOS
if(APPLE)
    find_library(METAL_FRAMEWORK Metal)
    find_library(FOUNDATION_FRAMEWORK Foundation)
    if(METAL_FRAMEWORK AND FOUNDATION_FRAMEWORK)
        target_link_libraries(llm-node-unit-tests PRIVATE ${METAL_FRAMEWORK} ${FOUNDATION_FRAMEWORK})
        target_compile_definitions(llm-node-unit-tests PRIVATE USE_METAL=1)
    endif()

    if(BUILD_WITH_GPTOSS)
        target_link_libraries(llm-node-unit-tests PRIVATE gptoss_metal)
        add_dependencies(llm-node-unit-tests gptoss_build_metallib)
        target_link_options(llm-node-unit-tests PRIVATE
            LINKER:-sectcreate,__METAL,__shaders,${CMAKE_BINARY_DIR}/${GPTOSS_METAL_LIB}
        )
    endif()
endif()

target_include_directories(llm-node-unit-tests
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/../../include
        ${CMAKE_CURRENT_SOURCE_DIR}/../../src
)

target_compile_definitions(llm-node-unit-tests
    PRIVATE
        LLM_NODE_TESTING=1
)

add_test(
    NAME llm-node-unit-tests
    COMMAND llm-node-unit-tests
)
