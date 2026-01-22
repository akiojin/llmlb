# リサーチ: OpenAI互換API完全準拠

**機能ID**: `SPEC-24157000` | **日付**: 2026-01-05

## 1. llama_tokenize API調査

### 決定

llama.cppの`llama_tokenize`関数を使用してトークン数を正確に計算する。

### 理由

- llama.cppに組み込まれており、追加依存関係不要
- モデル固有のトークナイザーを使用するため精度が高い
- 既にノードでモデルがロードされているため追加コストが低い

### API仕様

```cpp
int32_t llama_tokenize(
    const struct llama_model * model,
    const char * text,
    int32_t text_len,
    llama_token * tokens,
    int32_t n_max_tokens,
    bool add_bos,      // BOS(Begin of Sentence)トークン追加
    bool special       // 特殊トークンを認識
);
```

**戻り値**: トークン数（負の場合はバッファ不足）

### 使用パターン

```cpp
int count_tokens(const llama_model* model, const std::string& text) {
    // n_max_tokens=0でカウントのみ取得
    return -llama_tokenize(model, text.c_str(), text.length(), nullptr, 0, false, false);
}
```

### 検討した代替案

| 代替案 | 却下理由 |
|--------|----------|
| 文字数÷4概算 | 精度が低い（現在の実装） |
| tiktoken | 別ライブラリ依存、モデル互換性問題 |
| sentencepiece直接使用 | llama.cppが既にラップしている |

---

## 2. llama_get_logits API調査

### 決定

`llama_get_logits_ith`を使用して各トークン生成時のlogitsを取得し、softmaxでlog確率を計算する。

### 理由

- サンプリング直後のlogitsを取得可能
- 全語彙に対する確率分布を取得できる
- top_logprobsの実装に必要

### API仕様

```cpp
// 最後のトークンのlogitsを取得
float * llama_get_logits(struct llama_context * ctx);

// i番目のトークンのlogitsを取得
float * llama_get_logits_ith(struct llama_context * ctx, int32_t i);
```

**戻り値**: 語彙サイズの浮動小数点配列へのポインタ

### 使用パターン

```cpp
std::vector<LogprobInfo> compute_logprobs(
    llama_context* ctx,
    const std::vector<llama_token>& tokens,
    int top_logprobs
) {
    std::vector<LogprobInfo> result;
    for (size_t i = 0; i < tokens.size(); i++) {
        float* logits = llama_get_logits_ith(ctx, i);
        // softmax計算してlog確率に変換
        // top_logprobs個の上位トークンを抽出
    }
    return result;
}
```

### 検討した代替案

| 代替案 | 却下理由 |
|--------|----------|
| ダミー値（0.0固定） | 現在の実装、無意味 |
| サンプリング確率のみ | top_logprobs非対応 |

---

## 3. UUID生成ライブラリ調査

### 決定

タイムスタンプ + ランダム値のハイブリッド方式を採用。
外部ライブラリは使用せず、標準ライブラリのみで実装。

### 理由

- 追加依存関係なし
- 十分な一意性（ミリ秒精度 + 乱数）
- OpenAI形式「chatcmpl-xxx」に適合

### 実装パターン

```cpp
#include <chrono>
#include <random>
#include <sstream>
#include <iomanip>

std::string generate_response_id(const std::string& prefix) {
    auto now = std::chrono::system_clock::now();
    auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
        now.time_since_epoch()).count();

    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> dis(0, 0xFFFF);

    std::ostringstream oss;
    oss << prefix << "-"
        << std::hex << ms << "-"
        << std::setw(4) << std::setfill('0') << dis(gen);
    return oss.str();
}
```

### 検討した代替案

| 代替案 | 却下理由 |
|--------|----------|
| boost::uuid | 追加依存関係 |
| libuuid | Linux固有 |
| 純粋UUID v4 | 形式がOpenAIと異なる |

---

## 4. OpenAI API仕様確認

### presence_penalty / frequency_penalty

| パラメータ | 範囲 | デフォルト | 説明 |
|-----------|------|-----------|------|
| presence_penalty | -2.0 ~ 2.0 | 0.0 | 既出トークンへのペナルティ（存在のみ） |
| frequency_penalty | -2.0 ~ 2.0 | 0.0 | 既出トークンへのペナルティ（頻度比例） |

**llama.cppでの対応**:

- `presence_penalty`: `llama_sampler_add_penalties`のpenalty_present
- `frequency_penalty`: `llama_sampler_add_penalties`のpenalty_freq

### n パラメータ

| パラメータ | 範囲 | デフォルト | 説明 |
|-----------|------|-----------|------|
| n | 1 ~ 128 | 1 | 生成する候補数 |

**実装制約**: 推論時間がn倍になるため、上限を8に制限する。

### logprobs

| パラメータ | 型 | 説明 |
|-----------|------|------|
| logprobs | boolean | trueで確率情報を返す |
| top_logprobs | integer (0-20) | 上位候補数 |

**レスポンス形式**:

```json
{
  "logprobs": {
    "content": [
      {
        "token": "Hello",
        "logprob": -0.5,
        "top_logprobs": [
          {"token": "Hello", "logprob": -0.5},
          {"token": "Hi", "logprob": -1.2}
        ]
      }
    ]
  }
}
```

---

---

## 5. Open Responses API調査（2026-01-16追加）

### 決定

llmlbをロードバランサー/ゲートウェイとして機能させ、
Open Responses APIリクエストをバックエンドにパススルーする。

### 理由

- API変換は複雑性を大幅に増加させる
- ステートフル機能（previous_response_id）の管理が困難
- ツールコール変換も複雑
- パススルーであれば既存のproxy.rs機能を再利用可能

### アーキテクチャ決定（インタビュー結果）

| 決定項目 | 選択 | 理由 |
|----------|------|------|
| ロードバランサーの役割 | ロードバランサー/ゲートウェイ | シンプルさ優先 |
| API変換 | しない | 複雑性回避 |
| API優先度 | 両方対等に維持 | 移行強制なし |
| ステートフル機能 | バックエンドの責務 | ロードバランサーはステートレス |
| ツール実行 | クライアントの責務 | ロードバランサーは中継のみ |

### 実装決定

| 決定項目 | 選択 | 理由 |
|----------|------|------|
| エンドポイント | `/v1/responses` | OpenAI仕様準拠 |
| 認証方式 | 既存APIキー認証を共用 | 追加実装不要 |
| メトリクス | Chat APIと同等 | 一貫した監視 |
| ストリーミング | 完全パススルー | 変換不要 |
| 非対応バックエンド | 501 Not Implemented | 明確なエラー通知 |
| Capability通知 | /v1/modelsに追加 | クライアントが事前確認可能 |
| バックエンド検出 | ヘルスチェックで自動検出 | 運用負荷軽減 |

### プロバイダー対応状況

| プロバイダー | バージョン | 対応状況 | 検出方法 |
|-------------|-----------|----------|----------|
| Ollama | v0.13.3+ | ✅ 対応 | `/v1/responses` 200応答 |
| vLLM | 最新 | ✅ 対応 | `/v1/responses` 200応答 |
| OpenRouter | - | ✅ 対応 | `/v1/responses` 200応答 |
| xLLM | 計画中 | 別SPEC | - |

### 検討した代替案

| 代替案 | 却下理由 |
|--------|----------|
| API変換（Responses→Chat） | 複雑性大、ステート管理困難 |
| 認証分離 | ユーザー負担増 |
| ロードバランサーでステート管理 | スケール困難、単一障害点 |

---

## 結論

すべての技術的不明点が解決され、実装に必要なAPIと方針が確定した。

- 既存OpenAI互換API機能: llama.cpp APIを使用
- Open Responses API: パススルー方式で実装
