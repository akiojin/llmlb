#pragma once

#include <chrono>
#include <condition_variable>
#include <functional>
#include <mutex>
#include <thread>

namespace xllm {

/// TokenWatchdog: トークン間タイムアウト監視
/// ストリーミング生成中に、一定時間トークンが生成されない場合にタイムアウトをトリガーする。
/// kick() を呼ぶことでタイマーをリセットする。
class TokenWatchdog {
public:
    /// デフォルトコンストラクタ（デフォルトタイムアウト使用）
    TokenWatchdog();

    /// タイムアウトとコールバックを指定するコンストラクタ
    explicit TokenWatchdog(std::chrono::milliseconds timeout,
                           std::function<void()> on_timeout = {});

    ~TokenWatchdog();

    TokenWatchdog(const TokenWatchdog&) = delete;
    TokenWatchdog& operator=(const TokenWatchdog&) = delete;
    TokenWatchdog(TokenWatchdog&&) = delete;
    TokenWatchdog& operator=(TokenWatchdog&&) = delete;

    /// タイマーをリセットする（トークン生成時に呼び出す）
    void kick();

    /// ウォッチドッグを停止する（正常終了時に呼び出す）
    void stop();

    /// デフォルトタイムアウト（5秒、環境変数で上書き可能）
    static std::chrono::milliseconds defaultTimeout();

#ifdef XLLM_TESTING
    static void resetTestState();
    static bool wasTimeoutTriggered();
#endif

private:
    void run();
    void triggerTimeout();

    std::chrono::milliseconds timeout_;
    std::function<void()> on_timeout_;
    std::thread worker_;
    std::mutex mutex_;
    std::condition_variable cv_;
    bool stop_{false};
    bool kicked_{false};
};

}  // namespace xllm
