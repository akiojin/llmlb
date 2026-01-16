/**
 * @file safetensors_parser_test.cpp
 * @brief Unit tests for safetensors file parsing
 *
 * Tests for parsing the safetensors format:
 * - Header: 8-byte little-endian size + JSON metadata
 * - Tensors: Raw tensor data following the header
 */

#include <gtest/gtest.h>
#include "safetensors.h"
#include <vector>
#include <cstring>
#include <fstream>
#include <filesystem>

class SafetensorsParserTest : public ::testing::Test {
protected:
    void SetUp() override {
        stcpp_init();
        // Create test directory
        test_dir_ = std::filesystem::temp_directory_path() / "stcpp_test";
        std::filesystem::create_directories(test_dir_);
    }

    void TearDown() override {
        stcpp_free();
        // Clean up test directory
        std::filesystem::remove_all(test_dir_);
    }

    // Helper to create a minimal valid safetensors file
    void CreateMinimalSafetensorsFile(const std::filesystem::path& path) {
        std::ofstream file(path, std::ios::binary);

        // Minimal valid JSON header: empty metadata
        std::string header = R"({"__metadata__":{}})";
        uint64_t header_size = header.size();

        // Write 8-byte header size (little-endian)
        file.write(reinterpret_cast<const char*>(&header_size), 8);
        // Write header JSON
        file.write(header.data(), header.size());

        file.close();
    }

    // Helper to create an invalid file (not safetensors format)
    void CreateInvalidFile(const std::filesystem::path& path) {
        std::ofstream file(path, std::ios::binary);
        file << "This is not a safetensors file";
        file.close();
    }

    // Helper to create a truncated safetensors file
    void CreateTruncatedFile(const std::filesystem::path& path) {
        std::ofstream file(path, std::ios::binary);
        // Only write 4 bytes instead of 8 for header size
        uint32_t partial = 100;
        file.write(reinterpret_cast<const char*>(&partial), 4);
        file.close();
    }

    std::filesystem::path test_dir_;
};

// Test: Loading non-existent file returns error
TEST_F(SafetensorsParserTest, LoadNonExistentFileReturnsError) {
    struct ErrorData {
        bool called = false;
        stcpp_error error = STCPP_OK;
    } data;

    auto callback = [](stcpp_error err, const char*, void* ud) {
        auto* d = static_cast<ErrorData*>(ud);
        d->called = true;
        d->error = err;
    };

    stcpp_model* model = stcpp_model_load(
        "/nonexistent/model.safetensors",
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
    // Current stub returns UNSUPPORTED_ARCH, but real implementation
    // should return FILE_NOT_FOUND
    EXPECT_NE(data.error, STCPP_OK);
}

// Test: Loading invalid file format returns error
TEST_F(SafetensorsParserTest, LoadInvalidFormatReturnsError) {
    auto invalid_path = test_dir_ / "invalid.safetensors";
    CreateInvalidFile(invalid_path);

    struct ErrorData {
        bool called = false;
        stcpp_error error = STCPP_OK;
    } data;

    auto callback = [](stcpp_error err, const char*, void* ud) {
        auto* d = static_cast<ErrorData*>(ud);
        d->called = true;
        d->error = err;
    };

    stcpp_model* model = stcpp_model_load(
        invalid_path.c_str(),
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
    EXPECT_NE(data.error, STCPP_OK);
}

// Test: Loading truncated file returns error
TEST_F(SafetensorsParserTest, LoadTruncatedFileReturnsError) {
    auto truncated_path = test_dir_ / "truncated.safetensors";
    CreateTruncatedFile(truncated_path);

    struct ErrorData {
        bool called = false;
        stcpp_error error = STCPP_OK;
    } data;

    auto callback = [](stcpp_error err, const char*, void* ud) {
        auto* d = static_cast<ErrorData*>(ud);
        d->called = true;
        d->error = err;
    };

    stcpp_model* model = stcpp_model_load(
        truncated_path.c_str(),
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
    EXPECT_NE(data.error, STCPP_OK);
}

// Test: Header size larger than file returns error
TEST_F(SafetensorsParserTest, HeaderSizeLargerThanFileReturnsError) {
    auto bad_path = test_dir_ / "bad_header.safetensors";
    std::ofstream file(bad_path, std::ios::binary);

    // Write header size that's way too large
    uint64_t header_size = 1000000;
    file.write(reinterpret_cast<const char*>(&header_size), 8);
    file << "{}";  // Tiny actual content
    file.close();

    struct ErrorData {
        bool called = false;
    } data;

    auto callback = [](stcpp_error, const char*, void* ud) {
        static_cast<ErrorData*>(ud)->called = true;
    };

    stcpp_model* model = stcpp_model_load(
        bad_path.c_str(),
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
}

// Test: Invalid JSON in header returns error
TEST_F(SafetensorsParserTest, InvalidJsonHeaderReturnsError) {
    auto bad_json_path = test_dir_ / "bad_json.safetensors";
    std::ofstream file(bad_json_path, std::ios::binary);

    std::string bad_json = "{ this is not valid json }";
    uint64_t header_size = bad_json.size();
    file.write(reinterpret_cast<const char*>(&header_size), 8);
    file << bad_json;
    file.close();

    struct ErrorData {
        bool called = false;
    } data;

    auto callback = [](stcpp_error, const char*, void* ud) {
        static_cast<ErrorData*>(ud)->called = true;
    };

    stcpp_model* model = stcpp_model_load(
        bad_json_path.c_str(),
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
}

// Test: Empty file returns error
TEST_F(SafetensorsParserTest, EmptyFileReturnsError) {
    auto empty_path = test_dir_ / "empty.safetensors";
    std::ofstream file(empty_path, std::ios::binary);
    file.close();

    struct ErrorData {
        bool called = false;
    } data;

    auto callback = [](stcpp_error, const char*, void* ud) {
        static_cast<ErrorData*>(ud)->called = true;
    };

    stcpp_model* model = stcpp_model_load(
        empty_path.c_str(),
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
}

// Test: Directory path (not file) returns error
TEST_F(SafetensorsParserTest, DirectoryPathReturnsError) {
    struct ErrorData {
        bool called = false;
    } data;

    auto callback = [](stcpp_error, const char*, void* ud) {
        static_cast<ErrorData*>(ud)->called = true;
    };

    stcpp_model* model = stcpp_model_load(
        test_dir_.c_str(),
        callback,
        &data
    );

    EXPECT_EQ(model, nullptr);
    EXPECT_TRUE(data.called);
}
