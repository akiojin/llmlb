set(UNIT_TEST_SOURCES
    continuous_batch_scheduler_test.cpp
    gpu_detector_test.cpp
    vision_processor_test.cpp
    inference_cache_test.cpp
    prefix_cache_test.cpp
    replica_manager_test.cpp
    function_calling_test.cpp
    model_sync_test.cpp
    model_downloader_test.cpp
    model_storage_test.cpp
    inference_engine_test.cpp
    llama_engine_kv_cache_test.cpp
    engine_registry_test.cpp
    model_converter_test.cpp
    model_resolver_test.cpp
    llama_manager_test.cpp
    model_pool_test.cpp
    model_loader_test.cpp
    prefix_cache_test.cpp
    vision_processor_test.cpp
    replica_manager_test.cpp
    function_calling_test.cpp
    partial_load_guard_test.cpp
    onnx_tts_manager_test.cpp
    model_registry_test.cpp
    watchdog_test.cpp
    utils_config_test.cpp
    utils_misc_test.cpp
    utils_utf8_test.cpp
    request_guard_test.cpp
    resource_monitor_test.cpp
    cli_test.cpp
    progress_renderer_test.cpp
    ollama_compat_test.cpp
    repl_session_test.cpp
    ../../src/utils/cli.cpp
    ../../src/models/model_sync.cpp
    ../../src/models/model_downloader.cpp
    ../../src/models/model_storage.cpp
    ../../src/models/model_converter.cpp
    ../../src/models/model_resolver.cpp
    ../../src/core/llama_manager.cpp
    ../../src/core/model_pool.cpp
    ../../src/core/prefix_cache.cpp
    ../../src/core/engine_registry.cpp
    ../../src/core/continuous_batch_scheduler.cpp
    ../../src/core/llama_engine.cpp
    ../../src/core/inference_engine.cpp
    ../../src/core/text_manager.cpp
    ../../src/core/inference_cache.cpp
    ../../src/core/prefix_cache.cpp
    ../../src/core/replica_manager.cpp
    ../../src/core/request_watchdog.cpp
    ../../src/core/token_watchdog.cpp
    ../../src/core/vision_processor.cpp
    ../../src/core/replica_manager.cpp
    ../../src/core/function_calling.cpp
    ../../src/core/partial_load_guard.cpp
    ../../src/core/chat_template_renderer.cpp
    ../../src/api/openai_endpoints.cpp
    ../../src/api/node_endpoints.cpp
    ../../src/api/http_server.cpp
    ../../src/models/model_registry.cpp
    ../../src/metrics/prometheus_exporter.cpp
    ../../src/core/whisper_manager.cpp
    ../../src/core/onnx_tts_manager.cpp
    ../../src/core/sd_manager.cpp
    ../../src/runtime/state.cpp
    ../../src/cli/cli_client.cpp
    ../../src/cli/progress_renderer.cpp
    ../../src/cli/ollama_compat.cpp
    ../../src/cli/repl_session.cpp
)

# Add whisper tests only when whisper is enabled
if(BUILD_WITH_WHISPER)
    list(APPEND UNIT_TEST_SOURCES whisper_manager_test.cpp)
endif()
# Add sd tests only when sd is enabled
if(BUILD_WITH_SD)
    list(APPEND UNIT_TEST_SOURCES sd_manager_test.cpp)
endif()

# Use platform-specific gpu_detector source
if(APPLE)
    list(APPEND UNIT_TEST_SOURCES ../../src/system/gpu_detector.mm)
    list(APPEND UNIT_TEST_SOURCES ../../src/system/resource_monitor.mm)
else()
    list(APPEND UNIT_TEST_SOURCES ../../src/system/gpu_detector.cpp)
    list(APPEND UNIT_TEST_SOURCES ../../src/system/resource_monitor.cpp)
endif()

add_executable(allm-unit-tests
    ${UNIT_TEST_SOURCES}
)

target_link_libraries(allm-unit-tests
    PRIVATE
        test_common
        llama
        mtmd
        common
        nlohmann_json::nlohmann_json
        utils_lib
        OpenSSL::SSL
        OpenSSL::Crypto
)

if(BUILD_WITH_WHISPER)
    target_link_libraries(allm-unit-tests PRIVATE whisper)
endif()
if(BUILD_WITH_SD)
    target_link_libraries(allm-unit-tests PRIVATE stable-diffusion)
endif()
if(BUILD_WITH_ONNX)
    target_link_libraries(allm-unit-tests PRIVATE onnxruntime::onnxruntime)
endif()

# Link Metal frameworks on macOS
if(APPLE)
    find_library(METAL_FRAMEWORK Metal)
    find_library(FOUNDATION_FRAMEWORK Foundation)
    if(METAL_FRAMEWORK AND FOUNDATION_FRAMEWORK)
        target_link_libraries(allm-unit-tests PRIVATE ${METAL_FRAMEWORK} ${FOUNDATION_FRAMEWORK})
        target_compile_definitions(allm-unit-tests PRIVATE USE_METAL=1)
    endif()

endif()

target_include_directories(allm-unit-tests
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/../../include
        ${CMAKE_CURRENT_SOURCE_DIR}/../../src
)

target_compile_definitions(allm-unit-tests
    PRIVATE
        ALLM_TESTING=1
)

# Ensure whisper/sd manager implementations are compiled in unit tests
if(BUILD_WITH_WHISPER)
    target_compile_definitions(allm-unit-tests PRIVATE USE_WHISPER=1)
endif()
if(BUILD_WITH_SD)
    target_compile_definitions(allm-unit-tests PRIVATE USE_SD=1)
endif()

add_test(
    NAME allm-unit-tests
    COMMAND allm-unit-tests
)
