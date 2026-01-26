#pragma once

#ifdef _WIN32
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <random>
#include <string>
#include <chrono>

inline int setenv(const char* name, const char* value, int overwrite) {
    if (!name || !*name) {
        return -1;
    }
    if (!overwrite) {
        size_t len = 0;
        if (getenv_s(&len, nullptr, 0, name) == 0 && len > 0) {
            return 0;
        }
    }
    return _putenv_s(name, value ? value : "");
}

inline int unsetenv(const char* name) {
    if (!name || !*name) {
        return -1;
    }
    return _putenv_s(name, "");
}

inline char* mkdtemp(char* tmpl) {
    if (!tmpl) {
        return nullptr;
    }
    char* xs = std::strstr(tmpl, "XXXXXX");
    if (!xs) {
        return nullptr;
    }

    static const char kCharset[] = "abcdefghijklmnopqrstuvwxyz0123456789";
    std::mt19937 rng(static_cast<unsigned>(
        std::chrono::high_resolution_clock::now().time_since_epoch().count()));
    std::uniform_int_distribution<size_t> dist(0, sizeof(kCharset) - 2);

    for (int attempt = 0; attempt < 100; ++attempt) {
        for (int i = 0; i < 6; ++i) {
            xs[i] = kCharset[dist(rng)];
        }
        try {
            if (std::filesystem::create_directory(tmpl)) {
                return tmpl;
            }
        } catch (...) {
        }
    }

    return nullptr;
}
#endif
