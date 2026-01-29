#include "models/model_downloader.h"

#include <array>
#include <cstring>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <httplib.h>
#include <spdlog/spdlog.h>
#include <memory>
#include <regex>
#include <thread>
#include <vector>
#include <optional>
#include <algorithm>
#include <cctype>
#include <unordered_set>
#include <nlohmann/json.hpp>

#include "utils/config.h"
#include "utils/file_lock.h"
#include "utils/allowlist.h"
#include "utils/url_encode.h"
#include "models/model_storage.h"

namespace {

struct HttpUrl {
    std::string scheme;
    std::string host;
    int port{0};
    std::string path;
};

// Minimal SHA-256 implementation (public domain style)
struct Sha256Ctx {
    uint64_t bitlen = 0;
    uint32_t state[8];
    std::array<uint8_t, 64> data{};
    size_t datalen = 0;
};

constexpr uint32_t rotr(uint32_t x, uint32_t n) { return (x >> n) | (x << (32 - n)); }
constexpr uint32_t ch(uint32_t x, uint32_t y, uint32_t z) { return (x & y) ^ (~x & z); }
constexpr uint32_t maj(uint32_t x, uint32_t y, uint32_t z) { return (x & y) ^ (x & z) ^ (y & z); }
constexpr uint32_t ep0(uint32_t x) { return rotr(x, 2) ^ rotr(x, 13) ^ rotr(x, 22); }
constexpr uint32_t ep1(uint32_t x) { return rotr(x, 6) ^ rotr(x, 11) ^ rotr(x, 25); }
constexpr uint32_t sig0(uint32_t x) { return rotr(x, 7) ^ rotr(x, 18) ^ (x >> 3); }
constexpr uint32_t sig1(uint32_t x) { return rotr(x, 17) ^ rotr(x, 19) ^ (x >> 10); }

const uint32_t k[64] = {
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2};

void sha256_transform(Sha256Ctx& ctx, const uint8_t data[]) {
    uint32_t m[64];
    for (uint32_t i = 0, j = 0; i < 16; ++i, j += 4) {
        m[i] = (data[j] << 24) | (data[j + 1] << 16) | (data[j + 2] << 8) | (data[j + 3]);
    }
    for (uint32_t i = 16; i < 64; ++i) {
        m[i] = sig1(m[i - 2]) + m[i - 7] + sig0(m[i - 15]) + m[i - 16];
    }

    uint32_t a = ctx.state[0];
    uint32_t b = ctx.state[1];
    uint32_t c = ctx.state[2];
    uint32_t d = ctx.state[3];
    uint32_t e = ctx.state[4];
    uint32_t f = ctx.state[5];
    uint32_t g = ctx.state[6];
    uint32_t h = ctx.state[7];

    for (uint32_t i = 0; i < 64; ++i) {
        uint32_t t1 = h + ep1(e) + ch(e, f, g) + k[i] + m[i];
        uint32_t t2 = ep0(a) + maj(a, b, c);
        h = g;
        g = f;
        f = e;
        e = d + t1;
        d = c;
        c = b;
        b = a;
        a = t1 + t2;
    }

    ctx.state[0] += a;
    ctx.state[1] += b;
    ctx.state[2] += c;
    ctx.state[3] += d;
    ctx.state[4] += e;
    ctx.state[5] += f;
    ctx.state[6] += g;
    ctx.state[7] += h;
}

void sha256_init(Sha256Ctx& ctx) {
    ctx.datalen = 0;
    ctx.bitlen = 0;
    ctx.state[0] = 0x6a09e667;
    ctx.state[1] = 0xbb67ae85;
    ctx.state[2] = 0x3c6ef372;
    ctx.state[3] = 0xa54ff53a;
    ctx.state[4] = 0x510e527f;
    ctx.state[5] = 0x9b05688c;
    ctx.state[6] = 0x1f83d9ab;
    ctx.state[7] = 0x5be0cd19;
}

void sha256_update(Sha256Ctx& ctx, const uint8_t data[], size_t len) {
    for (size_t i = 0; i < len; ++i) {
        ctx.data[ctx.datalen] = data[i];
        ctx.datalen++;
        if (ctx.datalen == 64) {
            sha256_transform(ctx, ctx.data.data());
            ctx.bitlen += 512;
            ctx.datalen = 0;
        }
    }
}

std::array<uint8_t, 32> sha256_final(Sha256Ctx& ctx) {
    size_t i = ctx.datalen;

    // Pad whatever data is left in the buffer.
    if (ctx.datalen < 56) {
        ctx.data[i++] = 0x80;
        while (i < 56) ctx.data[i++] = 0x00;
    } else {
        ctx.data[i++] = 0x80;
        while (i < 64) ctx.data[i++] = 0x00;
        sha256_transform(ctx, ctx.data.data());
        memset(ctx.data.data(), 0, 56);
    }

    ctx.bitlen += ctx.datalen * 8;
    ctx.data[63] = static_cast<uint8_t>(ctx.bitlen);
    ctx.data[62] = static_cast<uint8_t>(ctx.bitlen >> 8);
    ctx.data[61] = static_cast<uint8_t>(ctx.bitlen >> 16);
    ctx.data[60] = static_cast<uint8_t>(ctx.bitlen >> 24);
    ctx.data[59] = static_cast<uint8_t>(ctx.bitlen >> 32);
    ctx.data[58] = static_cast<uint8_t>(ctx.bitlen >> 40);
    ctx.data[57] = static_cast<uint8_t>(ctx.bitlen >> 48);
    ctx.data[56] = static_cast<uint8_t>(ctx.bitlen >> 56);
    sha256_transform(ctx, ctx.data.data());

    std::array<uint8_t, 32> hash{};
    for (uint32_t j = 0; j < 4; ++j) {
        hash[j] = (ctx.state[0] >> (24 - j * 8)) & 0xff;
        hash[j + 4] = (ctx.state[1] >> (24 - j * 8)) & 0xff;
        hash[j + 8] = (ctx.state[2] >> (24 - j * 8)) & 0xff;
        hash[j + 12] = (ctx.state[3] >> (24 - j * 8)) & 0xff;
        hash[j + 16] = (ctx.state[4] >> (24 - j * 8)) & 0xff;
        hash[j + 20] = (ctx.state[5] >> (24 - j * 8)) & 0xff;
        hash[j + 24] = (ctx.state[6] >> (24 - j * 8)) & 0xff;
        hash[j + 28] = (ctx.state[7] >> (24 - j * 8)) & 0xff;
    }
    return hash;
}

std::string to_hex(const std::array<uint8_t, 32>& hash) {
    static const char* hex = "0123456789abcdef";
    std::string out;
    out.reserve(64);
    for (auto b : hash) {
        out.push_back(hex[(b >> 4) & 0x0f]);
        out.push_back(hex[b & 0x0f]);
    }
    return out;
}

std::string sha256_of_file(const std::filesystem::path& path) {
    Sha256Ctx ctx;
    sha256_init(ctx);

    std::ifstream ifs(path, std::ios::binary);
    if (!ifs.is_open()) return "";

    std::array<char, 4096> buf{};
    while (ifs) {
        ifs.read(buf.data(), buf.size());
        std::streamsize n = ifs.gcount();
        if (n > 0) sha256_update(ctx, reinterpret_cast<uint8_t*>(buf.data()), static_cast<size_t>(n));
    }

    return to_hex(sha256_final(ctx));
}

// Incremental SHA256 for streamed verification
class StreamingSha256 {
public:
    StreamingSha256() { sha256_init(ctx_); }
    void update(const char* data, size_t len) {
        sha256_update(ctx_, reinterpret_cast<const uint8_t*>(data), len);
    }
    std::string finalize() { return to_hex(sha256_final(ctx_)); }

private:
    Sha256Ctx ctx_;
};

HttpUrl parseUrl(const std::string& url) {
    static const std::regex re(R"(^([a-zA-Z][a-zA-Z0-9+.-]*)://([^/:]+)(?::(\d+))?(.*)$)");
    std::smatch match;
    HttpUrl parsed;
    if (std::regex_match(url, match, re)) {
        parsed.scheme = match[1].str();
        parsed.host = match[2].str();
        parsed.port = match[3].matched ? std::stoi(match[3].str()) : (parsed.scheme == "https" ? 443 : 80);
        parsed.path = match[4].str().empty() ? "/" : match[4].str();
    }
    return parsed;
}

std::optional<std::string> hfTokenForHost(const std::string& host) {
    const char* token = std::getenv("HF_TOKEN");
    if (!token || !*token) return std::nullopt;

    const auto lower = xllm::toLowerAscii(host);
    if (lower == "huggingface.co" || xllm::endsWith(lower, ".huggingface.co")) {
        return std::string(token);
    }

    const char* base = std::getenv("HF_BASE_URL");
    if (base && *base) {
        HttpUrl parsed = parseUrl(base);
        if (!parsed.host.empty()) {
            const auto base_lower = xllm::toLowerAscii(parsed.host);
            if (lower == base_lower) {
                return std::string(token);
            }
        }
    }

    return std::nullopt;
}

std::unique_ptr<httplib::Client> makeClient(const HttpUrl& url, std::chrono::milliseconds timeout) {
    if (url.scheme.empty() || url.host.empty()) {
        return nullptr;
    }

#ifndef CPPHTTPLIB_OPENSSL_SUPPORT
    if (url.scheme == "https") {
        return nullptr;  // HTTPS is not supported in this build
    }
#endif

    // Build scheme://host:port format for Client's universal interface
    std::string scheme_host_port = url.scheme + "://" + url.host;
    if (url.port != 0) {
        scheme_host_port += ":" + std::to_string(url.port);
    }

    auto client = std::make_unique<httplib::Client>(scheme_host_port);
    if (client && client->is_valid()) {
        const int sec = static_cast<int>(timeout.count() / 1000);
        const int usec = static_cast<int>((timeout.count() % 1000) * 1000);
        client->set_connection_timeout(sec, usec);
        client->set_read_timeout(sec, usec);
        client->set_write_timeout(sec, usec);
        client->set_follow_location(true);
        return client;
    }

    return nullptr;
}

std::string trimTrailingSlash(std::string value) {
    while (!value.empty() && value.back() == '/') {
        value.pop_back();
    }
    return value;
}

std::string normalizeBasePath(std::string path) {
    if (path == "/" || path.empty()) return "";
    if (!path.empty() && path.back() == '/') {
        path.pop_back();
    }
    return path;
}

std::string encodePathSegments(const std::string& path) {
    std::string out;
    size_t start = 0;
    while (start <= path.size()) {
        size_t pos = path.find('/', start);
        const std::string segment = (pos == std::string::npos) ? path.substr(start)
                                                               : path.substr(start, pos - start);
        out += xllm::urlEncodePathSegment(segment);
        if (pos == std::string::npos) break;
        out.push_back('/');
        start = pos + 1;
    }
    return out;
}

bool ends_with_case_insensitive(const std::string& value, const std::string& suffix) {
    if (value.size() < suffix.size()) return false;
    const size_t offset = value.size() - suffix.size();
    for (size_t i = 0; i < suffix.size(); ++i) {
        const auto lhs = static_cast<unsigned char>(value[offset + i]);
        const auto rhs = static_cast<unsigned char>(suffix[i]);
        if (std::tolower(lhs) != std::tolower(rhs)) return false;
    }
    return true;
}

bool is_gguf_filename(const std::string& filename) {
    return ends_with_case_insensitive(filename, ".gguf");
}

bool is_safetensors_index_filename(const std::string& filename) {
    return ends_with_case_insensitive(filename, ".safetensors.index.json");
}

bool is_safetensors_filename(const std::string& filename) {
    const auto lower = ends_with_case_insensitive(filename, ".safetensors") ||
                       ends_with_case_insensitive(filename, ".safetensors.index.json");
    return lower;
}

std::optional<std::string> infer_safetensors_index_from_shard(const std::string& filename) {
    if (is_safetensors_index_filename(filename)) {
        return std::nullopt;
    }
    if (!ends_with_case_insensitive(filename, ".safetensors")) {
        return std::nullopt;
    }

    std::string dir;
    std::string file = filename;
    auto slash = filename.find_last_of('/');
    if (slash != std::string::npos) {
        dir = filename.substr(0, slash + 1);
        file = filename.substr(slash + 1);
    }

    const std::string suffix = ".safetensors";
    if (file.size() <= suffix.size()) return std::nullopt;
    const std::string stem = file.substr(0, file.size() - suffix.size());

    auto pos = stem.rfind("-of-");
    if (pos == std::string::npos) return std::nullopt;
    const std::string left = stem.substr(0, pos);
    const std::string total = stem.substr(pos + 4);
    if (left.empty() || total.empty()) return std::nullopt;
    if (!std::all_of(total.begin(), total.end(),
                     [](unsigned char c) { return std::isdigit(c); })) {
        return std::nullopt;
    }

    auto pos2 = left.rfind('-');
    if (pos2 == std::string::npos) return std::nullopt;
    const std::string prefix = left.substr(0, pos2);
    const std::string shard = left.substr(pos2 + 1);
    if (prefix.empty() || shard.empty()) return std::nullopt;
    if (!std::all_of(shard.begin(), shard.end(),
                     [](unsigned char c) { return std::isdigit(c); })) {
        return std::nullopt;
    }

    return dir + prefix + ".safetensors.index.json";
}

bool has_sibling(const std::vector<std::string>& siblings, const std::string& filename) {
    return std::find(siblings.begin(), siblings.end(), filename) != siblings.end();
}

bool require_safetensors_metadata_files(const std::vector<std::string>& siblings) {
    return has_sibling(siblings, "config.json") && has_sibling(siblings, "tokenizer.json");
}

std::optional<std::string> resolve_safetensors_primary(const std::vector<std::string>& siblings,
                                                       const std::string& requested,
                                                       std::string& error) {
    if (!requested.empty()) {
        if (!is_safetensors_filename(requested)) {
            error = "filename must be a safetensors or safetensors index file";
            return std::nullopt;
        }
        if (!has_sibling(siblings, requested)) {
            error = "Specified safetensors file not found in repository";
            return std::nullopt;
        }
        if (!is_safetensors_index_filename(requested)) {
            if (auto candidate = infer_safetensors_index_from_shard(requested)) {
                if (has_sibling(siblings, *candidate)) {
                    return candidate;
                }
                std::vector<std::string> index_files;
                for (const auto& name : siblings) {
                    if (is_safetensors_index_filename(name)) {
                        index_files.push_back(name);
                    }
                }
                if (index_files.size() == 1) {
                    return index_files.front();
                }
                if (index_files.size() > 1) {
                    error = "Multiple safetensors index files found; specify filename";
                    return std::nullopt;
                }
            }
        }
        return requested;
    }

    std::vector<std::string> index_files;
    for (const auto& name : siblings) {
        if (is_safetensors_index_filename(name)) {
            index_files.push_back(name);
        }
    }
    if (index_files.size() == 1) return index_files.front();
    if (index_files.size() > 1) {
        error = "Multiple safetensors index files found; specify filename";
        return std::nullopt;
    }

    std::vector<std::string> st_files;
    for (const auto& name : siblings) {
        if (ends_with_case_insensitive(name, ".safetensors") && !is_safetensors_index_filename(name)) {
            st_files.push_back(name);
        }
    }
    if (st_files.size() == 1) return st_files.front();
    if (st_files.empty()) {
        error = "No safetensors file found in repository";
        return std::nullopt;
    }
    error = "Multiple safetensors files found; specify filename";
    return std::nullopt;
}

std::optional<std::string> find_metal_artifact(const std::vector<std::string>& siblings) {
    const char* candidates[] = {"model.metal.bin", "metal/model.bin"};
    for (const auto& name : candidates) {
        if (has_sibling(siblings, name)) {
            return std::string(name);
        }
    }
    return std::nullopt;
}

std::optional<int> manifest_file_priority(const std::string& name) {
    if (name == "config.json" || name == "tokenizer.json") return 10;
    if (is_safetensors_index_filename(name)) return 5;
    if (name == "model.metal.bin") return 5;
    return std::nullopt;
}

bool is_quantization_token(const std::string& token) {
    if (token.empty()) return false;
    for (char c : token) {
        if (!(std::isalnum(static_cast<unsigned char>(c)) || c == '_')) return false;
    }
    std::string upper = token;
    std::transform(upper.begin(), upper.end(), upper.begin(),
                   [](unsigned char c) { return static_cast<char>(std::toupper(c)); });
    auto has_digit = [](const std::string& value) {
        return std::any_of(value.begin(), value.end(),
                           [](unsigned char c) { return std::isdigit(c); });
    };
    auto starts_with_digit = [](const std::string& value) {
        return !value.empty() && std::isdigit(static_cast<unsigned char>(value[0]));
    };

    if (upper.rfind("IQ", 0) == 0) return starts_with_digit(upper.substr(2));
    if (upper.rfind("Q", 0) == 0) return starts_with_digit(upper.substr(1));
    if (upper.rfind("BF", 0) == 0) return starts_with_digit(upper.substr(2));
    if (upper.rfind("FP", 0) == 0) return starts_with_digit(upper.substr(2));
    if (upper.rfind("F", 0) == 0) return starts_with_digit(upper.substr(1));
    if (upper.rfind("MX", 0) == 0) return has_digit(upper.substr(2));
    return false;
}

std::optional<std::string> infer_quantization_from_filename(const std::string& filename) {
    if (!is_gguf_filename(filename)) return std::nullopt;
    auto slash = filename.find_last_of('/');
    std::string file = (slash == std::string::npos) ? filename : filename.substr(slash + 1);
    const std::string suffix = ".gguf";
    if (file.size() <= suffix.size()) return std::nullopt;
    const std::string stem = file.substr(0, file.size() - suffix.size());
    size_t pos = stem.size();
    while (pos > 0) {
        size_t prev = stem.find_last_of("-.", pos - 1);
        std::string token = (prev == std::string::npos) ? stem : stem.substr(prev + 1);
        if (is_quantization_token(token)) return token;
        if (prev == std::string::npos) break;
        pos = prev;
    }
    return std::nullopt;
}

std::string strip_gguf_extension(std::string filename) {
    const std::string suffix = ".gguf";
    if (ends_with_case_insensitive(filename, suffix) && filename.size() > suffix.size()) {
        filename.erase(filename.size() - suffix.size());
    }
    return filename;
}

std::string strip_quantization_suffix(std::string stem, const std::string& quant) {
    if (quant.empty()) return stem;
    const auto lower = xllm::toLowerAscii(stem);
    const auto quant_lower = xllm::toLowerAscii(quant);
    if (lower.size() <= quant_lower.size() + 1) return stem;
    const size_t pos = lower.rfind(quant_lower);
    if (pos == std::string::npos) return stem;
    if (pos + quant_lower.size() != lower.size()) return stem;
    const char sep = lower[pos - 1];
    if (sep != '-' && sep != '.') return stem;
    return stem.substr(0, pos - 1);
}

std::string infer_mmproj_base_key(const std::string& main_filename) {
    auto file = main_filename;
    auto slash = file.find_last_of("/\\");
    if (slash != std::string::npos) {
        file = file.substr(slash + 1);
    }
    file = strip_gguf_extension(file);
    if (auto quant = infer_quantization_from_filename(main_filename)) {
        file = strip_quantization_suffix(file, *quant);
    }
    return file;
}

std::optional<std::string> find_mmproj_artifact(const std::vector<std::string>& siblings,
                                                const std::string& main_filename) {
    std::vector<std::string> candidates;
    candidates.reserve(siblings.size());
    for (const auto& name : siblings) {
        if (!is_gguf_filename(name)) continue;
        const auto lower = xllm::toLowerAscii(name);
        if (lower.find("mmproj") == std::string::npos) continue;
        candidates.push_back(name);
    }
    if (candidates.empty()) return std::nullopt;

    const auto base_key = xllm::toLowerAscii(infer_mmproj_base_key(main_filename));
    if (!base_key.empty()) {
        for (const auto& candidate : candidates) {
            if (xllm::toLowerAscii(candidate).find(base_key) != std::string::npos) {
                return candidate;
            }
        }
    }

    std::sort(candidates.begin(), candidates.end());
    return candidates.front();
}

std::string get_hf_base_url() {
    const char* base = std::getenv("HF_BASE_URL");
    if (!base || !*base) {
        return "https://huggingface.co";
    }
    return trimTrailingSlash(std::string(base));
}

std::string build_hf_api_path(const HttpUrl& base, const std::string& repo) {
    const std::string prefix = normalizeBasePath(base.path);
    std::string path = prefix;
    path += "/api/models/" + encodePathSegments(repo);
    path += "?expand=siblings";
    if (path.empty() || path.front() != '/') {
        path.insert(path.begin(), '/');
    }
    return path;
}

std::string build_hf_resolve_path(const HttpUrl& base, const std::string& repo, const std::string& filename) {
    const std::string prefix = normalizeBasePath(base.path);
    std::string path = prefix;
    path += "/";
    path += encodePathSegments(repo);
    path += "/resolve/main/";
    path += encodePathSegments(filename);
    if (path.empty() || path.front() != '/') {
        path.insert(path.begin(), '/');
    }
    return path;
}

std::string build_hf_resolve_url(const std::string& base_url, const std::string& repo, const std::string& filename) {
    std::string out = trimTrailingSlash(base_url);
    out += "/";
    out += encodePathSegments(repo);
    out += "/resolve/main/";
    out += encodePathSegments(filename);
    return out;
}

bool validate_artifact_path(const std::string& path) {
    if (path.empty()) return false;
    if (path.find("..") != std::string::npos) return false;
    if (path.find('\0') != std::string::npos) return false;
    if (!path.empty() && (path.front() == '/' || path.front() == '\\')) return false;
    return true;
}

std::optional<std::string> find_signature_for(const std::vector<std::string>& siblings,
                                              const std::string& filename) {
    const std::string sig = filename + ".sig";
    if (std::find(siblings.begin(), siblings.end(), sig) != siblings.end()) {
        return sig;
    }
    const std::string asc = filename + ".asc";
    if (std::find(siblings.begin(), siblings.end(), asc) != siblings.end()) {
        return asc;
    }
    return std::nullopt;
}

struct HfMetadataCacheEntry {
    std::string etag;
    std::string body;
};

std::filesystem::path hf_metadata_cache_path(const std::string& models_dir) {
    return std::filesystem::path(models_dir) / ".hf_metadata_cache.json";
}

std::optional<HfMetadataCacheEntry> load_hf_metadata_cache(const std::string& models_dir,
                                                           const std::string& model_id) {
    const auto cache_path = hf_metadata_cache_path(models_dir);
    if (!std::filesystem::exists(cache_path)) return std::nullopt;

    xllm::FileLock lock(cache_path);
    if (!lock.locked()) return std::nullopt;

    std::ifstream ifs(cache_path, std::ios::binary);
    if (!ifs.is_open()) return std::nullopt;

    auto j = nlohmann::json::parse(ifs, nullptr, false);
    if (!j.is_object()) return std::nullopt;
    if (!j.contains(model_id) || !j[model_id].is_object()) return std::nullopt;

    const auto& entry = j[model_id];
    HfMetadataCacheEntry out;
    if (entry.contains("etag") && entry["etag"].is_string()) {
        out.etag = entry["etag"].get<std::string>();
    }
    if (entry.contains("body") && entry["body"].is_string()) {
        out.body = entry["body"].get<std::string>();
    }
    if (out.body.empty()) return std::nullopt;
    return out;
}

bool store_hf_metadata_cache(const std::string& models_dir,
                             const std::string& model_id,
                             const HfMetadataCacheEntry& entry) {
    if (entry.body.empty()) return false;

    const auto cache_path = hf_metadata_cache_path(models_dir);
    std::filesystem::create_directories(cache_path.parent_path());

    xllm::FileLock lock(cache_path);
    if (!lock.locked()) return false;

    nlohmann::json cache = nlohmann::json::object();
    if (std::filesystem::exists(cache_path)) {
        std::ifstream ifs(cache_path, std::ios::binary);
        auto current = nlohmann::json::parse(ifs, nullptr, false);
        if (current.is_object()) {
            cache = std::move(current);
        }
    }

    nlohmann::json cache_entry;
    if (!entry.etag.empty()) {
        cache_entry["etag"] = entry.etag;
    }
    cache_entry["body"] = entry.body;
    cache[model_id] = cache_entry;

    const auto temp_path = cache_path.string() + ".tmp";
    std::ofstream ofs(temp_path, std::ios::binary | std::ios::trunc);
    if (!ofs.is_open()) return false;
    ofs << cache.dump();
    ofs.flush();
    if (!ofs.good()) return false;

    std::error_code ec;
    std::filesystem::rename(temp_path, cache_path, ec);
    if (ec) return false;
    return true;
}

}  // namespace

namespace fs = std::filesystem;

namespace xllm {

ModelDownloader::ModelDownloader(std::string registry_base, std::string models_dir,
                                 std::chrono::milliseconds timeout, int max_retries,
                                 std::chrono::milliseconds backoff, std::string api_key)
    : registry_base_(std::move(registry_base)), models_dir_(std::move(models_dir)), timeout_(timeout),
      max_retries_(max_retries), backoff_(backoff), api_key_(std::move(api_key)) {

    // override by config
    auto cfg_pair = loadDownloadConfigWithLog();
    auto cfg = cfg_pair.first;
    log_source_ = cfg_pair.second;
    max_retries_ = cfg.max_retries;
    backoff_ = cfg.backoff;
    max_bytes_per_sec_ = cfg.max_bytes_per_sec;
    chunk_size_ = cfg.chunk_size;
}

std::string ModelDownloader::fetchManifest(const std::string& model_id, const std::string& filename_hint) {
    auto clear_error = [this]() {
        std::lock_guard<std::mutex> lock(error_mutex_);
        last_error_.clear();
    };
    auto set_error = [this](std::string msg) {
        std::lock_guard<std::mutex> lock(error_mutex_);
        last_error_ = std::move(msg);
    };
    clear_error();
    HttpUrl base = parseUrl(registry_base_);
    if (base.scheme.empty() || base.host.empty()) {
        return fetchHfManifest(model_id, filename_hint);
    }

    auto client = makeClient(base, timeout_);
    if (!client) {
        set_error("failed to create HTTP client for registry");
        spdlog::warn("ModelDownloader: failed to create HTTP client for '{}'", registry_base_);
        return "";
    }

    std::string path = base.path;
    if (path.empty()) path = "/";
    if (path.back() != '/') path.push_back('/');
    path += urlEncodePathSegment(model_id) + "/manifest.json";

    spdlog::info("ModelDownloader: fetching manifest url='{}{}'", registry_base_, path);

    const auto local_dir = xllm::ModelStorage::modelNameToDir(model_id);
    std::string out_path = models_dir_ + "/" + local_dir + "/manifest.json";
    fs::create_directories(models_dir_ + "/" + local_dir);
    FileLock lock(out_path + ".lock");
    // ロック取得できなくてもベストエフォートで進める
    httplib::Result res;
    for (int attempt = 0; attempt <= max_retries_; ++attempt) {
        if (!api_key_.empty()) {
            httplib::Headers headers = {{"Authorization", "Bearer " + api_key_}};
            res = client->Get(path.c_str(), headers);
        } else {
            res = client->Get(path.c_str());
        }
        if (res && res->status >= 200 && res->status < 300) break;
        if (attempt < max_retries_) std::this_thread::sleep_for(backoff_);
    }
    if (!res) {
        set_error("manifest request failed (no response)");
        spdlog::warn("ModelDownloader: manifest request failed (no response) url='{}{}'",
                     registry_base_, path);
        return "";
    }
    if (res->status < 200 || res->status >= 300) {
        set_error("manifest request failed status=" + std::to_string(res->status));
        spdlog::warn("ModelDownloader: manifest request failed status={} url='{}{}'",
                     res->status, registry_base_, path);
        return "";
    }
    spdlog::info("ModelDownloader: manifest response status={} bytes={}", res->status, res->body.size());
    if (res->body.empty()) {
        spdlog::warn("ModelDownloader: manifest response body empty url='{}{}'", registry_base_, path);
    }
    std::ofstream ofs(out_path, std::ios::binary | std::ios::trunc);
    if (!ofs.is_open()) {
        set_error("failed to open manifest file for write");
        spdlog::warn("ModelDownloader: failed to open manifest file for write path='{}'", out_path);
        return "";
    }
    ofs << res->body;
    ofs.flush();
    if (!ofs.good()) {
        set_error("failed to write manifest");
        spdlog::warn("ModelDownloader: failed to write manifest to path='{}'", out_path);
        return "";
    }
    // log applied config for diagnostics (opt-in)
    if (const char* logenv = std::getenv("LLM_DL_LOG_CONFIG")) {
        if (std::string(logenv) == "1" || std::string(logenv) == "true") {
            auto cfg_pair = loadDownloadConfigWithLog();
            auto cfg = cfg_pair.first;
            std::cerr << "[config] retries=" << cfg.max_retries
                      << " backoff_ms=" << cfg.backoff.count()
                      << " concurrency=" << cfg.max_concurrency
                      << " max_bps=" << cfg.max_bytes_per_sec
                      << " chunk=" << cfg.chunk_size
                      << " sources: " << cfg_pair.second << std::endl;
            if (cfg_pair.second.find("source=default") != std::string::npos) {
                std::cerr << "[config] using defaults (no env/file overrides)" << std::endl;
            }
        }
    }
    return out_path;
}

std::string ModelDownloader::fetchHfManifest(const std::string& model_id, const std::string& filename_hint) {
    auto clear_error = [this]() {
        std::lock_guard<std::mutex> lock(error_mutex_);
        last_error_.clear();
    };
    auto set_error = [this](std::string msg) {
        std::lock_guard<std::mutex> lock(error_mutex_);
        last_error_ = std::move(msg);
    };
    clear_error();
    if (model_id.empty()) {
        set_error("model id required");
        return "";
    }

    const std::string base_url = get_hf_base_url();
    HttpUrl base = parseUrl(base_url);
    if (base.scheme.empty() || base.host.empty()) {
        set_error("invalid HuggingFace base URL");
        spdlog::warn("ModelDownloader: invalid HuggingFace base URL '{}'", base_url);
        return "";
    }

    auto client = makeClient(base, timeout_);
    if (!client) {
        set_error("failed to create HTTP client for HuggingFace");
        spdlog::warn("ModelDownloader: failed to create HTTP client for '{}'", base_url);
        return "";
    }

    httplib::Headers headers;
    if (const char* token = std::getenv("HF_TOKEN")) {
        if (*token) headers.emplace("Authorization", std::string("Bearer ") + token);
    }

    std::optional<HfMetadataCacheEntry> cached_metadata = load_hf_metadata_cache(models_dir_, model_id);
    if (cached_metadata.has_value() && !cached_metadata->etag.empty()) {
        headers.emplace("If-None-Match", cached_metadata->etag);
    }

    const std::string api_path = build_hf_api_path(base, model_id);
    spdlog::info("ModelDownloader: fetching HuggingFace repo metadata path='{}'", api_path);

    auto res = client->Get(api_path.c_str(), headers);
    std::string metadata_body;
    if (!res) {
        if (!cached_metadata.has_value()) {
            set_error("HuggingFace request failed (no response)");
            spdlog::warn("ModelDownloader: HuggingFace request failed (no response) path='{}'", api_path);
            return "";
        }
        spdlog::warn("ModelDownloader: HuggingFace request failed, falling back to cached metadata path='{}'", api_path);
        metadata_body = cached_metadata->body;
    } else if (res->status == 304) {
        if (!cached_metadata.has_value()) {
            set_error("HuggingFace metadata cache missing for 304 response");
            spdlog::warn("ModelDownloader: HuggingFace metadata cache missing for 304 path='{}'", api_path);
            return "";
        }
        metadata_body = cached_metadata->body;
    } else if (res->status >= 200 && res->status < 300) {
        metadata_body = res->body;
        HfMetadataCacheEntry entry;
        entry.etag = res->get_header_value("ETag");
        entry.body = metadata_body;
        store_hf_metadata_cache(models_dir_, model_id, entry);
    } else {
        if (res->status == 401 || res->status == 403) {
            set_error("gated model or unauthorized (status=" + std::to_string(res->status) + ")");
        } else {
            set_error("HuggingFace request failed status=" + std::to_string(res->status));
        }
        spdlog::warn("ModelDownloader: HuggingFace request failed status={} path='{}'", res->status, api_path);
        return "";
    }

    auto body = nlohmann::json::parse(metadata_body, nullptr, false);
    if (body.is_discarded() || !body.contains("siblings") || !body["siblings"].is_array()) {
        set_error("HuggingFace response missing siblings");
        spdlog::warn("ModelDownloader: HuggingFace response missing siblings");
        return "";
    }

    std::vector<std::string> siblings;
    siblings.reserve(body["siblings"].size());
    for (const auto& entry : body["siblings"]) {
        if (entry.contains("rfilename") && entry["rfilename"].is_string()) {
            siblings.push_back(entry["rfilename"].get<std::string>());
        }
    }
    if (siblings.empty()) {
        set_error("HuggingFace response contains no files");
        spdlog::warn("ModelDownloader: HuggingFace response contains no files");
        return "";
    }

    std::string selection;
    enum class Format { Gguf, Safetensors };
    std::optional<Format> format;

    if (!filename_hint.empty()) {
        if (!has_sibling(siblings, filename_hint)) {
            set_error("Specified file not found in repository");
            return "";
        }
        if (is_gguf_filename(filename_hint)) {
            format = Format::Gguf;
            selection = filename_hint;
        } else if (is_safetensors_filename(filename_hint)) {
            if (!require_safetensors_metadata_files(siblings)) {
                set_error("config.json and tokenizer.json are required for safetensors models");
                return "";
            }
            std::string err;
            auto resolved = resolve_safetensors_primary(siblings, filename_hint, err);
            if (!resolved) {
                set_error(err.empty() ? "Failed to resolve safetensors index file" : err);
                return "";
            }
            format = Format::Safetensors;
            selection = *resolved;
        } else {
            set_error("filename must be a .gguf or .safetensors file");
            return "";
        }
    } else {
        std::vector<std::string> ggufs;
        std::vector<std::string> safetensors;
        for (const auto& name : siblings) {
            if (is_gguf_filename(name)) ggufs.push_back(name);
            if (is_safetensors_filename(name)) safetensors.push_back(name);
        }
        if (!ggufs.empty() && !safetensors.empty()) {
            set_error("Multiple artifact types found; specify filename");
            return "";
        }
        if (!ggufs.empty()) {
            if (ggufs.size() == 1) {
                format = Format::Gguf;
                selection = ggufs.front();
            } else {
                set_error("Multiple GGUF files found; specify filename");
                return "";
            }
        } else if (!safetensors.empty()) {
            if (!require_safetensors_metadata_files(siblings)) {
                set_error("config.json and tokenizer.json are required for safetensors models");
                return "";
            }
            std::string err;
            auto resolved = resolve_safetensors_primary(siblings, "", err);
            if (!resolved) {
                set_error(err.empty() ? "Failed to resolve safetensors index file" : err);
                return "";
            }
            format = Format::Safetensors;
            selection = *resolved;
        } else {
            set_error("No supported model artifacts found (safetensors/gguf)");
            return "";
        }
    }

    if (!format.has_value()) {
        set_error("Failed to resolve model format");
        return "";
    }

    nlohmann::json manifest;
    nlohmann::json files = nlohmann::json::array();

    bool ok = true;
    auto push_file = [&](const std::string& name, const std::string& url) {
        if (!validate_artifact_path(name)) {
            set_error("invalid artifact path: " + name);
            ok = false;
            return;
        }
        nlohmann::json entry;
        entry["name"] = name;
        entry["url"] = url;
        if (auto pr = manifest_file_priority(name)) {
            entry["priority"] = *pr;
        }
        if (auto sig = find_signature_for(siblings, name)) {
            nlohmann::json sig_entry;
            sig_entry["name"] = *sig;
            sig_entry["url"] = build_hf_resolve_url(base_url, model_id, *sig);
            entry["signature"] = sig_entry;
        }
        files.push_back(entry);
    };

    if (format && *format == Format::Gguf) {
        manifest["format"] = "gguf";
        if (auto quant = infer_quantization_from_filename(selection)) {
            manifest["quantization"] = *quant;
        }
        push_file("model.gguf", build_hf_resolve_url(base_url, model_id, selection));
        if (auto mmproj = find_mmproj_artifact(siblings, selection)) {
            if (*mmproj != selection) {
                push_file(*mmproj, build_hf_resolve_url(base_url, model_id, *mmproj));
            }
        }
    } else if (format && *format == Format::Safetensors) {
        manifest["format"] = "safetensors";
        std::vector<std::string> names = {"config.json", "tokenizer.json", selection};
        if (is_safetensors_index_filename(selection)) {
            const std::string index_path = selection;
            const std::string resolve_path = build_hf_resolve_path(base, model_id, index_path);
            auto index_res = client->Get(resolve_path.c_str(), headers);
            if (!index_res || index_res->status < 200 || index_res->status >= 300) {
                if (index_res && (index_res->status == 401 || index_res->status == 403)) {
                    set_error("gated model or unauthorized (status=" + std::to_string(index_res->status) + ")");
                } else {
                    set_error("Failed to fetch file: " + index_path);
                }
                return "";
            }
            auto index_json = nlohmann::json::parse(index_res->body, nullptr, false);
            if (index_json.is_discarded() || !index_json.contains("weight_map") || !index_json["weight_map"].is_object()) {
                set_error("Invalid safetensors index format");
                return "";
            }
            std::unordered_set<std::string> shard_set;
            for (auto it = index_json["weight_map"].begin(); it != index_json["weight_map"].end(); ++it) {
                if (it.value().is_string()) {
                    shard_set.insert(it.value().get<std::string>());
                }
            }
            std::vector<std::string> shards(shard_set.begin(), shard_set.end());
            std::sort(shards.begin(), shards.end());
            for (const auto& shard : shards) {
                if (std::find(names.begin(), names.end(), shard) == names.end()) {
                    names.push_back(shard);
                }
            }
        }

        for (const auto& name : names) {
            push_file(name, build_hf_resolve_url(base_url, model_id, name));
        }
    }

    if (!ok) {
        return "";
    }

    if (auto metal = find_metal_artifact(siblings)) {
        push_file("model.metal.bin", build_hf_resolve_url(base_url, model_id, *metal));
        if (!ok) {
            return "";
        }
    }

    manifest["files"] = files;

    const auto local_dir = xllm::ModelStorage::modelNameToDir(model_id);
    std::string out_path = models_dir_ + "/" + local_dir + "/manifest.json";
    fs::create_directories(models_dir_ + "/" + local_dir);
    FileLock lock(out_path + ".lock");

    std::ofstream ofs(out_path, std::ios::binary | std::ios::trunc);
    if (!ofs.is_open()) {
        set_error("failed to open manifest file for write");
        spdlog::warn("ModelDownloader: failed to open manifest file for write path='{}'", out_path);
        return "";
    }
    ofs << manifest.dump();
    ofs.flush();
    if (!ofs.good()) {
        set_error("failed to write manifest");
        spdlog::warn("ModelDownloader: failed to write manifest to path='{}'", out_path);
        return "";
    }

    return out_path;
}

std::string ModelDownloader::downloadBlob(const std::string& blob_url, const std::string& filename, ProgressCallback cb,
    const std::string& expected_sha256, const std::string& if_none_match) {
    auto clear_error = [this]() {
        std::lock_guard<std::mutex> lock(error_mutex_);
        last_error_.clear();
    };
    auto set_error = [this](std::string msg) {
        std::lock_guard<std::mutex> lock(error_mutex_);
        last_error_ = std::move(msg);
    };
    clear_error();
    HttpUrl url = parseUrl(blob_url);
    // Keep whether the original URL was relative before resolving against registry_base_.
    const bool is_relative = url.scheme.empty();

    // blob_url が相対パスの場合は registry_base_ を基準に解決する
    if (url.scheme.empty()) {
        url = parseUrl(registry_base_);
        if (url.scheme.empty()) {
            return "";
        }

        if (!blob_url.empty() && blob_url.front() == '/') {
            url.path = blob_url;
        } else {
            if (url.path.empty()) url.path = "/";
            if (url.path.back() != '/') url.path.push_back('/');
            url.path += blob_url;
        }
    }

    auto client = makeClient(url, timeout_);
    if (!client) {
        set_error("failed to create HTTP client for download");
        spdlog::warn("ModelDownloader: failed to create HTTP client for url='{}{}'",
                     url.scheme + "://" + url.host, url.path);
        return "";
    }

    const std::optional<std::string> hf_token = is_relative ? std::nullopt : hfTokenForHost(url.host);
    auto apply_auth = [&](httplib::Headers& headers) {
        if (is_relative) {
            if (!api_key_.empty()) {
                headers.emplace("Authorization", "Bearer " + api_key_);
            }
        } else if (hf_token.has_value()) {
            headers.emplace("Authorization", "Bearer " + *hf_token);
        }
    };

    fs::path out_path = fs::path(models_dir_) / filename;
    fs::create_directories(out_path.parent_path());

    // Prevent concurrent writers for the same blob (best-effort)
    FileLock blob_lock(out_path.string() + ".lock");

    const size_t original_offset = [&]() {
        if (!fs::exists(out_path)) return static_cast<size_t>(0);
        std::error_code ec;
        auto size = static_cast<size_t>(fs::file_size(out_path, ec));
        return ec ? static_cast<size_t>(0) : size;
    }();

    // If conditional ETag check is requested, avoid Range-based resume for simplicity.
    const size_t resume_offset = if_none_match.empty() ? original_offset : 0;
    const bool allow_range = if_none_match.empty();

    // If-None-Match handling: use simple GET and short-circuit 304 without streaming
    if (!if_none_match.empty()) {
        httplib::Headers hdrs{{"If-None-Match", if_none_match}};
        apply_auth(hdrs);
        for (int attempt = 0; attempt <= max_retries_; ++attempt) {
            auto res = client->Get(url.path, hdrs);
            if (res) {
                if (res->status == 304) {
                    if (fs::exists(out_path)) {
                        if (cb) cb(original_offset, original_offset);
                        return out_path.string();
                    }
                } else if (res->status >= 200 && res->status < 300) {
                    fs::create_directories(out_path.parent_path());
                    std::ofstream ofs(out_path, std::ios::binary | std::ios::trunc);
                    ofs << res->body;
                    ofs.flush();
                    if (cb) cb(res->body.size(), res->body.size());

                    if (!expected_sha256.empty()) {
                        auto actual = sha256_of_file(out_path);
                        if (actual.empty() || actual != expected_sha256) {
                            std::error_code ec;
                            fs::remove(out_path, ec);
                            return "";
                        }
                    }
                    return out_path.string();
                }
            }
            if (attempt < max_retries_) std::this_thread::sleep_for(backoff_);
        }
        if (fs::exists(out_path)) {
            // Assume not modified if server unreachable but cached file exists
            return out_path.string();
        }
        // could not satisfy conditional request
        set_error("download failed");
        return "";
    }

    auto download_once = [&](size_t offset, bool use_range) -> bool {
        for (int attempt = 0; attempt <= max_retries_; ++attempt) {
            std::ofstream ofs(out_path, std::ios::binary | (offset > 0 && use_range ? std::ios::app : std::ios::trunc));
            if (!ofs.is_open()) return false;

            size_t downloaded = offset;
            size_t total = offset;
            std::optional<StreamingSha256> streamer;
            if (!expected_sha256.empty()) streamer.emplace();

            auto start_time = std::chrono::steady_clock::now();

            httplib::Headers headers;
            apply_auth(headers);
            if (use_range && offset > 0) {
                headers.emplace("Range", "bytes=" + std::to_string(offset) + "-");
            }
            auto result = client->Get(
                url.path,
                headers,
                [&](const httplib::Response& res) {
                    if (res.has_header("Content-Length")) {
                        try {
                            total = offset + static_cast<size_t>(std::stoull(res.get_header_value("Content-Length")));
                        } catch (...) {
                            total = offset;
                        }
                    }
                    if (res.status == 304) {
                        // Not modified; treat as success if file already exists
                        return fs::exists(out_path);
                    }
                    return res.status >= 200 && res.status < 300;
                },
                [&](const char* data, size_t data_length) {
                    ofs.write(data, data_length);
                    downloaded += data_length;
                    if (streamer) streamer->update(data, data_length);
                    if (cb) cb(downloaded, total);

                    if (max_bytes_per_sec_ > 0) {
                        auto elapsed = std::chrono::steady_clock::now() - start_time;
                        double elapsed_sec = std::chrono::duration<double>(elapsed).count();
                        double allowed = max_bytes_per_sec_ * elapsed_sec;
                        if (downloaded > allowed && elapsed_sec > 0.0) {
                            double excess = downloaded - allowed;
                            double sleep_sec = excess / static_cast<double>(max_bytes_per_sec_);
                            if (sleep_sec > 0) {
                                std::this_thread::sleep_for(std::chrono::duration<double>(sleep_sec));
                            }
                        }
                    }
                    return true;
                });

            ofs.flush();

            if (result && (result->status == 304 || (result->status >= 200 && result->status < 300))) {
                if (cb && total == offset) {
                    cb(downloaded, downloaded);
                }
                if (streamer && !expected_sha256.empty() && result->status != 304) {
                    auto actual = streamer->finalize();
                    if (actual.empty() || actual != expected_sha256) {
                        std::error_code ec;
                        fs::remove(out_path, ec);
                        return false;
                    }
                }
                return true;
            }

            if (attempt < max_retries_) std::this_thread::sleep_for(backoff_);
        }
        return false;
    };

    // 1st attempt: resume with Range if partial exists
    auto validate_and_return = [&](bool resumed) -> std::string {
        if (expected_sha256.empty()) return out_path.string();
        auto actual = sha256_of_file(out_path);
        if (!actual.empty() && actual == expected_sha256) return out_path.string();

        // checksum mismatch
        if (resumed) {
            // retry full download once
            if (download_once(0, false)) {
                actual = sha256_of_file(out_path);
                if (!actual.empty() && actual == expected_sha256) return out_path.string();
            }
        }

        std::error_code ec;
        fs::remove(out_path, ec);
        set_error("download failed");
        return "";
    };

    if (download_once(resume_offset, allow_range)) {
        return validate_and_return(true);
    }

    // fallback: full re-download
    if (download_once(0, false)) {
        return validate_and_return(false);
    }

    // All attempts failed. Clean up only if the original file didn't exist.
    if (original_offset == 0) {
        std::error_code ec;
        fs::remove(out_path, ec);
    }
    set_error("download failed");
    return "";
}

}  // namespace xllm
