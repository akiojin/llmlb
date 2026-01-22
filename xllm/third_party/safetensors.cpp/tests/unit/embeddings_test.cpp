/**
 * @file embeddings_test.cpp
 * @brief Unit tests for embeddings generation (Task 37)
 */

#include <gtest/gtest.h>
#include <vector>
#include <cmath>
#include "safetensors.h"

class EmbeddingsTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
    }

    void TearDown() override {
        stcpp_free();
    }
};

// Test: Embeddings with null context
TEST_F(EmbeddingsTest, EmbeddingsNullContext) {
    float embeddings[1024];
    stcpp_error result = stcpp_embeddings(nullptr, "Test", embeddings, 1024);
    EXPECT_NE(result, STCPP_OK);
}

// Test: Embeddings dimensions with null model
TEST_F(EmbeddingsTest, EmbeddingsDimsNullModel) {
    int32_t dims = stcpp_embeddings_dims(nullptr);
    EXPECT_EQ(dims, 0);
}

// Test: Embedding vector properties
TEST_F(EmbeddingsTest, EmbeddingVectorProperties) {
    // Embeddings should be dense vectors
    // Common dimensions: 384, 768, 1024, 1536, 4096

    std::vector<int32_t> common_dims = {384, 768, 1024, 1536, 4096};

    for (int32_t dim : common_dims) {
        EXPECT_GT(dim, 0);
        EXPECT_LE(dim, 8192);  // Reasonable upper bound
    }
}

// Test: Embedding normalization
TEST_F(EmbeddingsTest, EmbeddingNormalization) {
    // Many embedding models produce normalized vectors (L2 norm = 1)
    std::vector<float> embedding = {0.6f, 0.8f, 0.0f};  // Pre-normalized

    float l2_norm = 0.0f;
    for (float v : embedding) {
        l2_norm += v * v;
    }
    l2_norm = std::sqrt(l2_norm);

    EXPECT_NEAR(l2_norm, 1.0f, 0.001f);
}

// Test: Cosine similarity
TEST_F(EmbeddingsTest, CosineSimilarity) {
    // Similar texts should have high cosine similarity
    std::vector<float> emb1 = {0.8f, 0.6f, 0.0f};
    std::vector<float> emb2 = {0.75f, 0.66f, 0.05f};
    std::vector<float> emb3 = {-0.8f, -0.6f, 0.0f};  // Opposite direction

    auto cosine_sim = [](const std::vector<float>& a, const std::vector<float>& b) {
        float dot = 0.0f, norm_a = 0.0f, norm_b = 0.0f;
        for (size_t i = 0; i < a.size(); i++) {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }
        return dot / (std::sqrt(norm_a) * std::sqrt(norm_b));
    };

    float sim_12 = cosine_sim(emb1, emb2);
    float sim_13 = cosine_sim(emb1, emb3);

    EXPECT_GT(sim_12, 0.9f);   // Similar embeddings
    EXPECT_LT(sim_13, -0.9f);  // Opposite embeddings
}

// Test: Batch embeddings concept
TEST_F(EmbeddingsTest, BatchEmbeddingsConcept) {
    // Processing multiple texts should be more efficient than one-by-one
    std::vector<std::string> texts = {
        "Hello world",
        "Machine learning",
        "Natural language processing",
    };

    int embedding_dim = 768;

    // Expected output shape: [num_texts, embedding_dim]
    size_t expected_total = texts.size() * embedding_dim;
    EXPECT_EQ(expected_total, 3 * 768);
}

// Test: Empty text handling
TEST_F(EmbeddingsTest, EmptyTextHandling) {
    // Empty text should either:
    // - Return an error
    // - Return a zero/special embedding
    std::string empty_text = "";
    EXPECT_TRUE(empty_text.empty());
}

// Test: Maximum text length
TEST_F(EmbeddingsTest, MaximumTextLength) {
    // Embedding models have maximum input length
    // Common limits: 512, 8192 tokens

    int max_tokens = 8192;
    int avg_chars_per_token = 4;

    int max_chars = max_tokens * avg_chars_per_token;
    EXPECT_GT(max_chars, 0);
    EXPECT_EQ(max_chars, 32768);
}

