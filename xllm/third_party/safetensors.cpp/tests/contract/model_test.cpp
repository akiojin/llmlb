/**
 * @file model_test.cpp
 * @brief Contract tests for model operations
 */

#include <gtest/gtest.h>
#include "safetensors.h"
#include <string>

class ModelTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Test: Model load with non-existent path returns nullptr
TEST_F(ModelTest, LoadNonExistentPathReturnsNull) {
    stcpp_model* model = stcpp_model_load(
        "/nonexistent/path/to/model",
        nullptr,
        nullptr
    );
    EXPECT_EQ(model, nullptr);
}

// Test: Model load with nullptr path returns nullptr
TEST_F(ModelTest, LoadNullPathReturnsNull) {
    stcpp_model* model = stcpp_model_load(nullptr, nullptr, nullptr);
    EXPECT_EQ(model, nullptr);
}

// Test: Model load calls error callback on failure
TEST_F(ModelTest, LoadCallsErrorCallbackOnFailure) {
    struct CallbackData {
        bool called = false;
        stcpp_error error = STCPP_OK;
        std::string message;
    } data;

    auto callback = [](stcpp_error error, const char* msg, void* user_data) {
        auto* d = static_cast<CallbackData*>(user_data);
        d->called = true;
        d->error = error;
        if (msg) {
            d->message = msg;
        }
    };

    stcpp_model* model = stcpp_model_load(
        "/nonexistent/path",
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
    // Error should indicate file not found or unsupported
    EXPECT_NE(data.error, STCPP_OK);
}

// Test: Model free with nullptr does not crash
TEST_F(ModelTest, FreeNullptrDoesNotCrash) {
    EXPECT_NO_THROW(stcpp_model_free(nullptr));
}

// Test: Model name returns nullptr for nullptr model
TEST_F(ModelTest, NameReturnsNullForNullModel) {
    const char* name = stcpp_model_name(nullptr);
    EXPECT_EQ(name, nullptr);
}

// Test: Model layer count returns 0 for nullptr model
TEST_F(ModelTest, LayerCountReturnsZeroForNullModel) {
    int32_t layers = stcpp_model_n_layers(nullptr);
    EXPECT_EQ(layers, 0);
}

// Test: Model head count returns 0 for nullptr model
TEST_F(ModelTest, HeadCountReturnsZeroForNullModel) {
    int32_t heads = stcpp_model_n_heads(nullptr);
    EXPECT_EQ(heads, 0);
}

// Test: Model hidden size returns 0 for nullptr model
TEST_F(ModelTest, HiddenSizeReturnsZeroForNullModel) {
    int32_t hidden = stcpp_model_hidden_size(nullptr);
    EXPECT_EQ(hidden, 0);
}

// Test: Model vocab size returns 0 for nullptr model
TEST_F(ModelTest, VocabSizeReturnsZeroForNullModel) {
    int32_t vocab = stcpp_model_vocab_size(nullptr);
    EXPECT_EQ(vocab, 0);
}

// Test: Model max context returns 0 for nullptr model
TEST_F(ModelTest, MaxContextReturnsZeroForNullModel) {
    int32_t max_ctx = stcpp_model_max_context(nullptr);
    EXPECT_EQ(max_ctx, 0);
}

// Test: VRAM estimate returns reasonable defaults for non-existent model
TEST_F(ModelTest, VramEstimateForNonExistentModel) {
    stcpp_vram_estimate estimate = stcpp_model_estimate_vram(
        "/nonexistent/path",
        STCPP_BACKEND_METAL,
        0
    );

    // For non-existent model, can_load should be false
    EXPECT_FALSE(estimate.can_load);
}

// Test: VRAM estimate with nullptr path
TEST_F(ModelTest, VramEstimateWithNullPath) {
    stcpp_vram_estimate estimate = stcpp_model_estimate_vram(
        nullptr,
        STCPP_BACKEND_METAL,
        0
    );

    EXPECT_FALSE(estimate.can_load);
}

// Test: Model embedding dims returns 0 for nullptr model
TEST_F(ModelTest, EmbeddingDimsReturnsZeroForNullModel) {
    int32_t dims = stcpp_embeddings_dims(nullptr);
    EXPECT_EQ(dims, 0);
}

// Test: Get tokenizer from nullptr model returns nullptr
TEST_F(ModelTest, GetTokenizerFromNullModelReturnsNull) {
    stcpp_tokenizer* tokenizer = stcpp_model_get_tokenizer(nullptr);
    EXPECT_EQ(tokenizer, nullptr);
}
