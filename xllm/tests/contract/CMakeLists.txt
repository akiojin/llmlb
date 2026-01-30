set(CONTRACT_TEST_SOURCES
    openai_api_test.cpp
    audio_speech_stream_test.cpp
    image_url_test.cpp
    cli_serve_test.cpp
    cli_run_test.cpp
    cli_pull_test.cpp
    cli_list_test.cpp
    cli_show_test.cpp
    cli_rm_test.cpp
    cli_stop_test.cpp
    cli_ps_test.cpp
    cli_profile_test.cpp
    cli_benchmark_test.cpp
    cli_compare_test.cpp
    cli_convert_test.cpp
    cli_export_import_test.cpp
    ../../src/api/http_server.cpp
    ../../src/api/openai_endpoints.cpp
    ../../src/api/node_endpoints.cpp
    ../../src/api/audio_endpoints.cpp
    ../../src/api/image_endpoints.cpp
    ../../src/utils/pgp.cpp
    ../../src/models/model_registry.cpp
    ../../src/models/modelfile.cpp
    ../../src/models/model_sync.cpp
    ../../src/models/model_downloader.cpp
    ../../src/models/model_storage.cpp
    ../../src/models/model_resolver.cpp
    ../../src/metrics/metrics_state.cpp
    ../../src/metrics/prometheus_exporter.cpp
    ../../src/core/engine_registry.cpp
    ../../src/core/lora_utils.cpp
    ../../src/core/chat_template_renderer.cpp
    ../../src/core/llama_engine.cpp
    ../../src/core/kv_cache_utils.cpp
    ../../src/core/inference_engine.cpp
    ../../src/core/text_manager.cpp
    ../../src/core/request_watchdog.cpp
    ../../src/core/token_watchdog.cpp
    ../../src/core/vision_processor.cpp
    ../../src/core/llama_manager.cpp
    ../../src/core/whisper_manager.cpp
    ../../src/core/onnx_tts_manager.cpp
    ../../src/core/sd_manager.cpp
    ../../src/runtime/state.cpp
)

if(APPLE)
    list(APPEND CONTRACT_TEST_SOURCES ../../src/system/gpu_detector.mm)
    list(APPEND CONTRACT_TEST_SOURCES ../../src/system/resource_monitor.mm)
else()
    list(APPEND CONTRACT_TEST_SOURCES ../../src/system/gpu_detector.cpp)
    list(APPEND CONTRACT_TEST_SOURCES ../../src/system/resource_monitor.cpp)
endif()

add_executable(xllm-contract-tests
    ${CONTRACT_TEST_SOURCES}
)

target_link_libraries(xllm-contract-tests
    PRIVATE
        test_common
        llama
        mtmd
        common
        nlohmann_json::nlohmann_json
        utils_lib
        cli_lib
        OpenSSL::SSL
        OpenSSL::Crypto
        ZLIB::ZLIB
)

# Ensure whisper/sd/onnx manager implementations are linked when enabled
if(BUILD_WITH_WHISPER)
    target_link_libraries(xllm-contract-tests PRIVATE whisper)
endif()
if(BUILD_WITH_SD)
    target_link_libraries(xllm-contract-tests PRIVATE stable-diffusion)
endif()
if(BUILD_WITH_ONNX)
    target_link_libraries(xllm-contract-tests PRIVATE onnxruntime::onnxruntime)
endif()

target_include_directories(xllm-contract-tests
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/../../include
        ${CMAKE_CURRENT_SOURCE_DIR}/../../src
)

# Enable testing mode for model registry (skip supported model filtering)
target_compile_definitions(xllm-contract-tests PRIVATE XLLM_TESTING=1)

if(BUILD_WITH_WHISPER)
    target_compile_definitions(xllm-contract-tests PRIVATE USE_WHISPER=1)
endif()
if(BUILD_WITH_SD)
    target_compile_definitions(xllm-contract-tests PRIVATE USE_SD=1)
endif()

# Link Metal frameworks on macOS
if(APPLE)
    find_library(METAL_FRAMEWORK Metal)
    find_library(FOUNDATION_FRAMEWORK Foundation)
    if(METAL_FRAMEWORK AND FOUNDATION_FRAMEWORK)
        target_link_libraries(xllm-contract-tests PRIVATE ${METAL_FRAMEWORK} ${FOUNDATION_FRAMEWORK})
        target_compile_definitions(xllm-contract-tests PRIVATE USE_METAL=1)
    endif()
endif()

add_test(
    NAME xllm-contract-tests
    COMMAND xllm-contract-tests
)
