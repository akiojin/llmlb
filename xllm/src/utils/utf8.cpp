#include "utils/utf8.h"

namespace xllm {

namespace {
constexpr char kReplacement[] = "\xEF\xBF\xBD";

inline void append_replacement(std::string& out) {
    out.append(kReplacement, sizeof(kReplacement) - 1);
}

inline bool is_continuation(unsigned char c) {
    return (c & 0xC0) == 0x80;
}

}  // namespace

std::string sanitize_utf8_lossy(std::string_view input) {
    std::string out;
    out.reserve(input.size());

    const auto* bytes = reinterpret_cast<const unsigned char*>(input.data());
    size_t i = 0;
    while (i < input.size()) {
        const unsigned char c0 = bytes[i];
        if (c0 <= 0x7F) {
            out.push_back(static_cast<char>(c0));
            ++i;
            continue;
        }

        size_t len = 0;
        if (c0 >= 0xC2 && c0 <= 0xDF) {
            len = 2;
        } else if (c0 >= 0xE0 && c0 <= 0xEF) {
            len = 3;
        } else if (c0 >= 0xF0 && c0 <= 0xF4) {
            len = 4;
        } else {
            append_replacement(out);
            ++i;
            continue;
        }

        if (i + len > input.size()) {
            append_replacement(out);
            break;
        }

        const unsigned char c1 = bytes[i + 1];
        if (!is_continuation(c1)) {
            append_replacement(out);
            ++i;
            continue;
        }

        if (len == 3 || len == 4) {
            const unsigned char c2 = bytes[i + 2];
            if (!is_continuation(c2)) {
                append_replacement(out);
                ++i;
                continue;
            }
        }
        if (len == 4) {
            const unsigned char c3 = bytes[i + 3];
            if (!is_continuation(c3)) {
                append_replacement(out);
                ++i;
                continue;
            }
        }

        // Prevent overlong sequences / surrogates / out-of-range.
        bool ok = true;
        if (len == 3) {
            if (c0 == 0xE0 && c1 < 0xA0) ok = false;  // overlong
            if (c0 == 0xED && c1 >= 0xA0) ok = false;  // surrogate
        } else if (len == 4) {
            if (c0 == 0xF0 && c1 < 0x90) ok = false;  // overlong
            if (c0 == 0xF4 && c1 > 0x8F) ok = false;  // > U+10FFFF
        }

        if (!ok) {
            append_replacement(out);
            ++i;
            continue;
        }

        out.append(input.data() + i, len);
        i += len;
    }

    return out;
}

}  // namespace xllm

