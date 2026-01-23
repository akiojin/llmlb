/**
 * @file sampling.cpp
 * @brief Token sampling implementation (Task 28)
 */

#include "safetensors_internal.h"
#include <cmath>
#include <algorithm>
#include <random>
#include <vector>
#include <numeric>

namespace stcpp {

/* Sampling context for internal use */
struct SamplingContext {
    std::mt19937 rng;
    stcpp_sampling_params params;
    std::vector<int32_t> prev_tokens;  // For repeat penalty

    explicit SamplingContext(const stcpp_sampling_params& p)
        : params(p) {
        if (p.seed >= 0) {
            rng.seed(static_cast<uint32_t>(p.seed));
        } else {
            std::random_device rd;
            rng.seed(rd());
        }
    }
};

/* Apply temperature to logits */
void apply_temperature(std::vector<float>& logits, float temperature) {
    if (temperature <= 0.0f) {
        // Greedy: set max to very high, rest to very low
        auto max_it = std::max_element(logits.begin(), logits.end());
        for (auto& l : logits) {
            l = (&l == &(*max_it)) ? 1e10f : -1e10f;
        }
        return;
    }

    if (temperature != 1.0f) {
        for (auto& l : logits) {
            l /= temperature;
        }
    }
}

/* Apply repeat penalty to logits */
void apply_repeat_penalty(
    std::vector<float>& logits,
    const std::vector<int32_t>& prev_tokens,
    float repeat_penalty
) {
    if (repeat_penalty == 1.0f || prev_tokens.empty()) {
        return;
    }

    for (int32_t token : prev_tokens) {
        if (token >= 0 && static_cast<size_t>(token) < logits.size()) {
            if (logits[token] > 0) {
                logits[token] /= repeat_penalty;
            } else {
                logits[token] *= repeat_penalty;
            }
        }
    }
}

/* Apply presence penalty */
void apply_presence_penalty(
    std::vector<float>& logits,
    const std::vector<int32_t>& prev_tokens,
    float presence_penalty
) {
    if (presence_penalty == 0.0f || prev_tokens.empty()) {
        return;
    }

    // Create a set of tokens that have appeared
    std::vector<bool> appeared(logits.size(), false);
    for (int32_t token : prev_tokens) {
        if (token >= 0 && static_cast<size_t>(token) < logits.size()) {
            appeared[token] = true;
        }
    }

    for (size_t i = 0; i < logits.size(); i++) {
        if (appeared[i]) {
            logits[i] -= presence_penalty;
        }
    }
}

/* Apply frequency penalty */
void apply_frequency_penalty(
    std::vector<float>& logits,
    const std::vector<int32_t>& prev_tokens,
    float frequency_penalty
) {
    if (frequency_penalty == 0.0f || prev_tokens.empty()) {
        return;
    }

    // Count token frequencies
    std::vector<int32_t> counts(logits.size(), 0);
    for (int32_t token : prev_tokens) {
        if (token >= 0 && static_cast<size_t>(token) < logits.size()) {
            counts[token]++;
        }
    }

    for (size_t i = 0; i < logits.size(); i++) {
        if (counts[i] > 0) {
            logits[i] -= frequency_penalty * static_cast<float>(counts[i]);
        }
    }
}

/* Apply softmax to logits */
void softmax(std::vector<float>& logits) {
    float max_val = *std::max_element(logits.begin(), logits.end());

    float sum = 0.0f;
    for (auto& l : logits) {
        l = std::exp(l - max_val);
        sum += l;
    }

    if (sum > 0.0f) {
        for (auto& l : logits) {
            l /= sum;
        }
    }
}

/* Apply top-k filtering */
void apply_top_k(std::vector<float>& probs, int32_t top_k) {
    if (top_k <= 0 || static_cast<size_t>(top_k) >= probs.size()) {
        return;
    }

    // Find k-th largest element
    std::vector<float> sorted = probs;
    std::partial_sort(sorted.begin(), sorted.begin() + top_k, sorted.end(),
                      std::greater<float>());
    float threshold = sorted[top_k - 1];

    // Zero out elements below threshold
    for (auto& p : probs) {
        if (p < threshold) {
            p = 0.0f;
        }
    }
}

/* Apply top-p (nucleus) filtering */
void apply_top_p(std::vector<float>& probs, float top_p) {
    if (top_p >= 1.0f) {
        return;
    }

    // Create index-probability pairs
    std::vector<std::pair<size_t, float>> indexed_probs;
    indexed_probs.reserve(probs.size());
    for (size_t i = 0; i < probs.size(); i++) {
        indexed_probs.emplace_back(i, probs[i]);
    }

    // Sort by probability descending
    std::sort(indexed_probs.begin(), indexed_probs.end(),
              [](const auto& a, const auto& b) { return a.second > b.second; });

    // Find cutoff
    float cumsum = 0.0f;
    size_t cutoff = indexed_probs.size();
    for (size_t i = 0; i < indexed_probs.size(); i++) {
        cumsum += indexed_probs[i].second;
        if (cumsum >= top_p) {
            cutoff = i + 1;
            break;
        }
    }

    // Zero out elements after cutoff
    for (size_t i = cutoff; i < indexed_probs.size(); i++) {
        probs[indexed_probs[i].first] = 0.0f;
    }
}

/* Apply min-p filtering */
void apply_min_p(std::vector<float>& probs, float min_p) {
    if (min_p <= 0.0f) {
        return;
    }

    float max_prob = *std::max_element(probs.begin(), probs.end());
    float threshold = max_prob * min_p;

    for (auto& p : probs) {
        if (p < threshold) {
            p = 0.0f;
        }
    }
}

/* Sample a token from probability distribution */
int32_t sample_token(std::vector<float>& probs, std::mt19937& rng) {
    // Normalize probabilities
    float sum = std::accumulate(probs.begin(), probs.end(), 0.0f);
    if (sum <= 0.0f) {
        // Fallback to argmax
        return static_cast<int32_t>(
            std::max_element(probs.begin(), probs.end()) - probs.begin());
    }

    for (auto& p : probs) {
        p /= sum;
    }

    // Sample
    std::uniform_real_distribution<float> dist(0.0f, 1.0f);
    float r = dist(rng);

    float cumsum = 0.0f;
    for (size_t i = 0; i < probs.size(); i++) {
        cumsum += probs[i];
        if (r < cumsum) {
            return static_cast<int32_t>(i);
        }
    }

    // Fallback: return last token
    return static_cast<int32_t>(probs.size() - 1);
}

/* Main sampling function */
int32_t sample_next_token(
    const float* logits,
    int32_t vocab_size,
    const stcpp_sampling_params& params,
    const std::vector<int32_t>& prev_tokens,
    std::mt19937& rng
) {
    // Copy logits to working vector
    std::vector<float> working(logits, logits + vocab_size);

    // Apply penalties
    apply_repeat_penalty(working, prev_tokens, params.repeat_penalty);
    apply_presence_penalty(working, prev_tokens, params.presence_penalty);
    apply_frequency_penalty(working, prev_tokens, params.frequency_penalty);

    // Apply temperature
    apply_temperature(working, params.temperature);

    // Convert to probabilities
    softmax(working);

    // Apply filtering
    apply_top_k(working, params.top_k);
    apply_top_p(working, params.top_p);
    apply_min_p(working, params.min_p);

    // Sample
    return sample_token(working, rng);
}

}  // namespace stcpp
