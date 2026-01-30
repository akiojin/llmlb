#include <gtest/gtest.h>

#include <filesystem>
#include <fstream>
#include <vector>
#include <string>

#include "models/modelfile.h"

namespace fs = std::filesystem;

namespace {
class EnvGuard {
public:
    EnvGuard(const std::string& key, const std::string& value) : key_(key) {
        const char* prev = std::getenv(key.c_str());
        if (prev) {
            had_prev_ = true;
            prev_value_ = prev;
        }
#ifdef _WIN32
        _putenv_s(key.c_str(), value.c_str());
#else
        setenv(key.c_str(), value.c_str(), 1);
#endif
    }

    ~EnvGuard() {
#ifdef _WIN32
        if (had_prev_) {
            _putenv_s(key_.c_str(), prev_value_.c_str());
        } else {
            _putenv_s(key_.c_str(), "");
        }
#else
        if (had_prev_) {
            setenv(key_.c_str(), prev_value_.c_str(), 1);
        } else {
            unsetenv(key_.c_str());
        }
#endif
    }

private:
    std::string key_;
    bool had_prev_{false};
    std::string prev_value_;
};

fs::path make_temp_dir() {
    auto base = fs::temp_directory_path() / fs::path("modelfile-test-XXXXXX");
    std::string tmpl = base.string();
    std::vector<char> buf(tmpl.begin(), tmpl.end());
    buf.push_back('\0');
    char* created = mkdtemp(buf.data());
    return created ? fs::path(created) : fs::temp_directory_path();
}
}

TEST(ModelfileTest, ParsesDirectives) {
    const fs::path temp_dir = make_temp_dir();
    EnvGuard home_guard("HOME", temp_dir.string());

    const std::string model_name = "gpt-oss-7b";
    const fs::path path = xllm::Modelfile::pathForModel(model_name);
    fs::create_directories(path.parent_path());

    const std::string content =
        "FROM base-model\n"
        "PARAMETER temperature 0.2\n"
        "PARAMETER stop \"STOP\"\n"
        "SYSTEM \"\"\"You are system\"\"\"\n"
        "TEMPLATE \"\"\"{{ messages[0].content }}\"\"\"\n"
        "MESSAGE user \"hello\"\n";

    std::ofstream ofs(path);
    ofs << content;
    ofs.close();

    std::string error;
    auto modelfile = xllm::Modelfile::loadForModel(model_name, error);
    ASSERT_TRUE(modelfile.has_value());
    EXPECT_EQ(modelfile->base_model, "base-model");
    EXPECT_EQ(modelfile->system_prompt, "You are system");
    EXPECT_EQ(modelfile->template_text, "{{ messages[0].content }}");
    ASSERT_EQ(modelfile->messages.size(), 1u);
    EXPECT_EQ(modelfile->messages[0].role, "user");
    EXPECT_EQ(modelfile->messages[0].content, "hello");
    ASSERT_TRUE(modelfile->parameters.count("temperature"));
    EXPECT_EQ(modelfile->parameters.at("temperature"), "0.2");
    ASSERT_TRUE(modelfile->parameters.count("stop"));
    EXPECT_EQ(modelfile->parameters.at("stop"), "STOP");
}
