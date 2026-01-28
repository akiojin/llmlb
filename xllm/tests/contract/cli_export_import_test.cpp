#include <gtest/gtest.h>
#include "utils/cli.h"

using namespace xllm;

class CliExportImportTest : public ::testing::Test {
protected:
    void SetUp() override {
        unsetenv("LLMLB_HOST");
    }
};

TEST_F(CliExportImportTest, ExportRequiresModelAndOutput) {
    const char* argv[] = {"xllm", "export", "llama3"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 1);
    EXPECT_NE(result.output.find("output"), std::string::npos);
}

TEST_F(CliExportImportTest, ExportParsesModelAndOutput) {
    const char* argv[] = {"xllm", "export", "llama3", "--output", "model.json"};
    auto result = parseCliArgs(5, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Export);
    EXPECT_EQ(result.export_options.model, "llama3");
    EXPECT_EQ(result.export_options.output, "model.json");
}

TEST_F(CliExportImportTest, ImportRequiresModelAndFile) {
    const char* argv[] = {"xllm", "import", "llama3"};
    auto result = parseCliArgs(3, const_cast<char**>(argv));

    EXPECT_TRUE(result.should_exit);
    EXPECT_EQ(result.exit_code, 1);
    EXPECT_NE(result.output.find("file"), std::string::npos);
}

TEST_F(CliExportImportTest, ImportParsesModelAndFile) {
    const char* argv[] = {"xllm", "import", "llama3", "--file", "Modelfile"};
    auto result = parseCliArgs(5, const_cast<char**>(argv));

    EXPECT_FALSE(result.should_exit);
    EXPECT_EQ(result.subcommand, Subcommand::Import);
    EXPECT_EQ(result.import_options.model, "llama3");
    EXPECT_EQ(result.import_options.file, "Modelfile");
}
