// Allowlist utilities for external model downloads.
#pragma once

#include <algorithm>
#include <cctype>
#include <regex>
#include <string>
#include <vector>

namespace xllm {

inline std::string toLowerAscii(std::string s) {
    std::transform(s.begin(), s.end(), s.begin(), [](unsigned char c) {
        return static_cast<char>(std::tolower(c));
    });
    return s;
}

inline std::string trimAscii(const std::string& s) {
    size_t start = 0;
    size_t end = s.size();
    while (start < end && std::isspace(static_cast<unsigned char>(s[start]))) ++start;
    while (end > start && std::isspace(static_cast<unsigned char>(s[end - 1]))) --end;
    return s.substr(start, end - start);
}

inline bool endsWith(const std::string& value, const std::string& suffix) {
    return value.size() >= suffix.size() &&
           value.compare(value.size() - suffix.size(), suffix.size(), suffix) == 0;
}

inline std::vector<std::string> splitAllowlistCsv(const std::string& csv) {
    std::vector<std::string> out;
    std::string cur;
    for (char c : csv) {
        if (c == ',') {
            auto token = trimAscii(cur);
            if (!token.empty()) out.push_back(token);
            cur.clear();
            continue;
        }
        cur.push_back(c);
    }
    auto token = trimAscii(cur);
    if (!token.empty()) out.push_back(token);
    return out;
}

inline std::string globToRegex(const std::string& pattern) {
    std::string out;
    out.reserve(pattern.size() * 2);
    out.push_back('^');
    for (char c : pattern) {
        switch (c) {
            case '*':
                out.append(".*");
                break;
            case '.':
            case '\\':
            case '+':
            case '?':
            case '(':
            case ')':
            case '[':
            case ']':
            case '{':
            case '}':
            case '^':
            case '$':
            case '|':
                out.push_back('\\');
                out.push_back(c);
                break;
            default:
                out.push_back(c);
        }
    }
    out.push_back('$');
    return out;
}

inline bool globMatch(const std::string& pattern, const std::string& value) {
    if (pattern == "*") return true;
    try {
        std::regex re(globToRegex(pattern), std::regex::icase);
        return std::regex_match(value, re);
    } catch (...) {
        return false;
    }
}

struct ParsedUrl {
    std::string scheme;
    std::string host;
    std::string path;
};

inline ParsedUrl parseUrlSimple(const std::string& url) {
    static const std::regex re(R"(^([a-zA-Z][a-zA-Z0-9+.-]*)://([^/:]+)(?::(\d+))?(.*)$)");
    std::smatch match;
    ParsedUrl parsed;
    if (std::regex_match(url, match, re)) {
        parsed.scheme = match[1].str();
        parsed.host = match[2].str();
        parsed.path = match[4].str().empty() ? "/" : match[4].str();
    }
    return parsed;
}

inline std::string extractHfRepo(const std::string& host, const std::string& path) {
    const auto host_lower = toLowerAscii(host);
    static const std::string hf_host = "huggingface.co";
    static const std::string hf_suffix = ".huggingface.co";
    if (host_lower != hf_host && !endsWith(host_lower, hf_suffix)) return "";

    if (path.empty() || path[0] != '/') return "";
    std::vector<std::string> segments;
    std::string cur;
    for (size_t i = 1; i < path.size(); ++i) {
        char c = path[i];
        if (c == '/') {
            if (!cur.empty()) {
                segments.push_back(cur);
                cur.clear();
            }
            continue;
        }
        cur.push_back(c);
    }
    if (!cur.empty()) segments.push_back(cur);
    if (segments.size() < 2) return "";
    if (segments[0] == "api") return "";
    return toLowerAscii(segments[0] + "/" + segments[1]);
}

inline bool isUrlAllowedByAllowlist(const std::string& url, const std::vector<std::string>& allowlist) {
    if (allowlist.empty()) return false;
    auto parsed = parseUrlSimple(url);
    if (parsed.scheme.empty() || parsed.host.empty()) return false;

    const std::string scheme = toLowerAscii(parsed.scheme);
    const std::string host = toLowerAscii(parsed.host);
    const std::string path = parsed.path.empty() ? "/" : parsed.path;
    const std::string full_url = scheme + "://" + host + path;
    const std::string host_path = host + path;
    const std::string repo = extractHfRepo(host, path);

    for (const auto& raw : allowlist) {
        auto pat = trimAscii(raw);
        if (pat.empty()) continue;
        pat = toLowerAscii(pat);
        if (pat == "*") return true;
        if (pat.find("://") != std::string::npos) {
            if (globMatch(pat, full_url)) return true;
            continue;
        }
        if (pat.find('.') != std::string::npos) {
            if (globMatch(pat, host_path) || globMatch(pat, host)) return true;
            continue;
        }
        if (pat.find('/') != std::string::npos) {
            if (!repo.empty() && globMatch(pat, repo)) return true;
            continue;
        }
        if (globMatch(pat, host)) return true;
    }
    return false;
}

}  // namespace xllm
