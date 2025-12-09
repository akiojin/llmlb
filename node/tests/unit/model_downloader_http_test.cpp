#include <gtest/gtest.h>
#include <httplib.h>
#include <filesystem>
#include <thread>

#include "models/model_downloader.h"

using namespace llm_node;

TEST(ModelDownloaderHttpTest, DownloadsAbsoluteHttpUrl) {
    // Start simple HTTP server
    httplib::Server svr;
    svr.Get("/file", [](const httplib::Request&, httplib::Response& res) {
        res.set_content("ok", "application/octet-stream");
    });

    const char* host = "127.0.0.1";
    const int port = 18189;  // fixed test port
    std::thread server_thread([&]() { svr.listen(host, port); });
    std::this_thread::sleep_for(std::chrono::milliseconds(100));

    auto temp_dir = std::filesystem::temp_directory_path() / "model-dl-http-test";
    std::filesystem::remove_all(temp_dir);

    ModelDownloader downloader(
        "http://127.0.0.1:18189",
        temp_dir.string(),
        std::chrono::milliseconds(5000),
        /*max_retries=*/0,
        std::chrono::milliseconds(100));

    auto out = downloader.downloadBlob("http://127.0.0.1:18189/file", "test/model.gguf");

    svr.stop();
    server_thread.join();

    ASSERT_FALSE(out.empty());
    ASSERT_TRUE(std::filesystem::exists(out));
    std::ifstream ifs(out, std::ios::binary);
    std::string body((std::istreambuf_iterator<char>(ifs)), std::istreambuf_iterator<char>());
    EXPECT_EQ(body, "ok");
}
