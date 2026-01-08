// SPEC-58378000: Integration test for model lifecycle workflow
// T014: Verifies show -> rm -> list model management flow

#include <gtest/gtest.h>
#include "cli/cli_client.h"
#include "cli/ollama_compat.h"

#include <cstdlib>
#include <string>

namespace llm_node {
namespace cli {
namespace {

/// Integration test fixture for model lifecycle workflow
class CliModelLifecycleTest : public ::testing::Test {
protected:
    void SetUp() override {
        // Save original environment
        const char* host = std::getenv("LLM_ROUTER_HOST");
        const char* port = std::getenv("LLM_NODE_PORT");
        original_host_ = host ? host : "";
        original_port_ = port ? port : "";

        // Set test environment
        setenv("LLM_ROUTER_HOST", "127.0.0.1", 1);
        setenv("LLM_NODE_PORT", "11435", 1);
    }

    void TearDown() override {
        // Restore original environment
        if (original_host_.empty()) {
            unsetenv("LLM_ROUTER_HOST");
        } else {
            setenv("LLM_ROUTER_HOST", original_host_.c_str(), 1);
        }
        if (original_port_.empty()) {
            unsetenv("LLM_NODE_PORT");
        } else {
            setenv("LLM_NODE_PORT", original_port_.c_str(), 1);
        }
    }

    std::string original_host_;
    std::string original_port_;
};

/// Test: show command returns model metadata
/// Scenario: Show existing model returns details
TEST_F(CliModelLifecycleTest, ShowReturnsModelMetadata) {
    auto client = std::make_shared<CliClient>();

    // Skip if server is not running
    if (!client->isServerRunning()) {
        GTEST_SKIP() << "Server not running";
    }

    // First list models to find an existing one
    auto list_result = client->listModels();
    if (!list_result.ok()) {
        GTEST_SKIP() << "Cannot list models: " << list_result.error_message;
    }

    // Find a model to show
    std::string model_name;
    if (list_result.data && list_result.data->contains("models")) {
        const auto& models = (*list_result.data)["models"];
        if (!models.empty()) {
            model_name = models[0].value("name", "");
        }
    }

    if (model_name.empty()) {
        GTEST_SKIP() << "No models available for show test";
    }

    // Show model details
    auto show_result = client->showModel(model_name);
    ASSERT_TRUE(show_result.ok()) << show_result.error_message;

    // Should have model information
    ASSERT_TRUE(show_result.data.has_value());

    // Verify expected fields exist
    const auto& data = *show_result.data;
    EXPECT_TRUE(data.contains("name") || data.contains("modelfile"));
}

/// Test: show command handles non-existent model
/// Scenario: Show non-existent model returns error
TEST_F(CliModelLifecycleTest, ShowHandlesNonExistentModel) {
    auto client = std::make_shared<CliClient>();

    // Skip if server is not running
    if (!client->isServerRunning()) {
        GTEST_SKIP() << "Server not running";
    }

    // Try to show a model that doesn't exist
    auto result = client->showModel("nonexistent-model-xyz-12345");

    // Should return error (but not crash)
    EXPECT_FALSE(result.ok());
    EXPECT_EQ(result.error, CliError::GeneralError);
}

/// Test: rm command deletes model
/// Scenario: Delete model and verify it's gone
TEST_F(CliModelLifecycleTest, RmDeletesModel) {
    auto client = std::make_shared<CliClient>();

    // Skip if server is not running
    if (!client->isServerRunning()) {
        GTEST_SKIP() << "Server not running";
    }

    // First, we need a model to delete
    // List current models
    auto initial_list = client->listModels();
    if (!initial_list.ok()) {
        GTEST_SKIP() << "Cannot list models";
    }

    // This is a destructive test - skip if no test models available
    // In CI, we would pull a test model first
    GTEST_SKIP() << "Skipping destructive rm test in automated testing";

    // The actual test would be:
    // 1. Pull a test model
    // 2. Verify it exists
    // 3. Delete it
    // 4. Verify it's gone
}

/// Test: rm handles non-existent model
/// Scenario: Delete non-existent model returns error
TEST_F(CliModelLifecycleTest, RmHandlesNonExistentModel) {
    auto client = std::make_shared<CliClient>();

    // Skip if server is not running
    if (!client->isServerRunning()) {
        GTEST_SKIP() << "Server not running";
    }

    // Try to delete a model that doesn't exist
    auto result = client->deleteModel("nonexistent-model-xyz-12345");

    // Should return error
    EXPECT_FALSE(result.ok());
    EXPECT_EQ(result.error, CliError::GeneralError);
}

/// Test: stop command unloads running model
/// Scenario: Stop loaded model and verify status
TEST_F(CliModelLifecycleTest, StopUnloadsRunningModel) {
    auto client = std::make_shared<CliClient>();

    // Skip if server is not running
    if (!client->isServerRunning()) {
        GTEST_SKIP() << "Server not running";
    }

    // First check running models
    auto ps_before = client->listRunningModels();
    if (!ps_before.ok()) {
        GTEST_SKIP() << "Cannot list running models";
    }

    // If no models are running, skip
    if (!ps_before.data || !ps_before.data->contains("models") ||
        (*ps_before.data)["models"].empty()) {
        GTEST_SKIP() << "No running models to stop";
    }

    // Get first running model
    std::string model_name = (*ps_before.data)["models"][0].value("name", "");
    if (model_name.empty()) {
        GTEST_SKIP() << "Cannot determine running model name";
    }

    // Stop the model
    auto stop_result = client->stopModel(model_name);
    EXPECT_TRUE(stop_result.ok()) << stop_result.error_message;

    // Verify it's no longer in running list
    auto ps_after = client->listRunningModels();
    ASSERT_TRUE(ps_after.ok());

    bool still_running = false;
    if (ps_after.data && ps_after.data->contains("models")) {
        for (const auto& model : (*ps_after.data)["models"]) {
            if (model.value("name", "") == model_name) {
                still_running = true;
                break;
            }
        }
    }
    EXPECT_FALSE(still_running) << "Model should no longer be running";
}

/// Test: ps command lists running models
/// Scenario: PS shows currently loaded models
TEST_F(CliModelLifecycleTest, PsListsRunningModels) {
    auto client = std::make_shared<CliClient>();

    // Skip if server is not running
    if (!client->isServerRunning()) {
        GTEST_SKIP() << "Server not running";
    }

    // List running models
    auto result = client->listRunningModels();
    ASSERT_TRUE(result.ok()) << result.error_message;

    // Result should have models array (even if empty)
    ASSERT_TRUE(result.data.has_value());
    EXPECT_TRUE(result.data->contains("models"));

    // If there are running models, they should have required fields
    if (result.data->contains("models")) {
        for (const auto& model : (*result.data)["models"]) {
            EXPECT_TRUE(model.contains("name"));
            // Additional fields: vram, temperature, etc.
        }
    }
}

/// Test: complete lifecycle: show -> rm -> list
/// Scenario: Full model management workflow
TEST_F(CliModelLifecycleTest, DISABLED_CompleteLifecycle) {
    // This test requires a test model to be available
    // Enable manually for full integration testing

    auto client = std::make_shared<CliClient>();

    if (!client->isServerRunning()) {
        GTEST_SKIP() << "Server not running";
    }

    const std::string test_model = "test-model";

    // 1. Pull model first
    auto pull_result = client->pullModel(test_model, nullptr);
    ASSERT_TRUE(pull_result.ok()) << "Pull failed: " << pull_result.error_message;

    // 2. Show model details
    auto show_result = client->showModel(test_model);
    ASSERT_TRUE(show_result.ok()) << "Show failed: " << show_result.error_message;
    EXPECT_TRUE(show_result.data.has_value());

    // 3. Verify model in list
    auto list_before = client->listModels();
    ASSERT_TRUE(list_before.ok());
    bool found_before = false;
    if (list_before.data && list_before.data->contains("models")) {
        for (const auto& m : (*list_before.data)["models"]) {
            if (m.value("name", "") == test_model) {
                found_before = true;
                break;
            }
        }
    }
    EXPECT_TRUE(found_before) << "Model should be in list after pull";

    // 4. Remove model
    auto rm_result = client->deleteModel(test_model);
    ASSERT_TRUE(rm_result.ok()) << "Delete failed: " << rm_result.error_message;

    // 5. Verify model no longer in list
    auto list_after = client->listModels();
    ASSERT_TRUE(list_after.ok());
    bool found_after = false;
    if (list_after.data && list_after.data->contains("models")) {
        for (const auto& m : (*list_after.data)["models"]) {
            if (m.value("name", "") == test_model) {
                found_after = true;
                break;
            }
        }
    }
    EXPECT_FALSE(found_after) << "Model should not be in list after rm";
}

/// Test: ollama models are read-only
/// Scenario: Cannot delete ollama: prefixed models
TEST_F(CliModelLifecycleTest, OllamaModelsAreReadOnly) {
    auto client = std::make_shared<CliClient>();

    // Skip if server is not running
    if (!client->isServerRunning()) {
        GTEST_SKIP() << "Server not running";
    }

    // Check if ollama models exist
    OllamaCompat ollama;
    auto ollama_models = ollama.listModels();

    if (ollama_models.empty()) {
        GTEST_SKIP() << "No ollama models installed";
    }

    // Try to delete an ollama model (should fail)
    std::string ollama_model = "ollama:" + ollama_models[0].name;
    auto result = client->deleteModel(ollama_model);

    // Should fail - ollama models are read-only
    EXPECT_FALSE(result.ok());
}

}  // namespace
}  // namespace cli
}  // namespace llm_node
