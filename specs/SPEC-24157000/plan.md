# 実装計画: OpenAI互換API完全準拠

**機能ID**: `SPEC-24157000` | **日付**: 2026-01-05 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-24157000/spec.md`の機能仕様

## 概要

内蔵エンジン（llama.cppベースノード）のOpenAI互換APIを100%準拠にする。
主要な改善項目:

1. **usage計算**: llama_tokenize APIを使用した正確なトークン数計算
2. **レスポンスID**: UUID/タイムスタンプベースの一意ID生成
3. **ペナルティパラメータ**: presence_penalty/frequency_penalty対応
4. **logprobs**: llama_get_logits APIを使用した実確率値計算
5. **nパラメータ**: 複数候補生成対応

## 技術コンテキスト

**言語/バージョン**: C++17 (node), Rust 1.75+ (router)
**主要依存関係**: llama.cpp (トークナイザー、サンプリング、logits取得)
**ストレージ**: N/A
**テスト**: Google Test (node), cargo test (router)
**対象プラットフォーム**: Linux, macOS, Windows
**プロジェクトタイプ**: single (既存node/router構造を維持)
**パフォーマンス目標**: 既存のレイテンシを維持（トークン計算オーバーヘッド < 1ms）
**制約**: llama.cpp API依存、後方互換性維持
**スケール/スコープ**: 既存APIの拡張（破壊的変更なし）

## 憲章チェック

**シンプルさ**:

- プロジェクト数: 2 (node, router) ✅
- フレームワークを直接使用? ✅ llama.cpp APIを直接使用
- 単一データモデル? ✅ OpenAI互換JSON形式
- パターン回避? ✅ 追加パターンなし

**アーキテクチャ**:

- すべての機能をライブラリとして? ✅ node/src/api/内
- ライブラリリスト: openai_endpoints.cpp (OpenAI互換API実装)
- ライブラリごとのCLI: N/A (APIサーバー)
- ライブラリドキュメント: N/A

**テスト (妥協不可)**:

- RED-GREEN-Refactorサイクルを強制? ✅
- Gitコミットはテストが実装より先に表示? ✅
- 順序: Contract→Integration→E2E→Unitを厳密に遵守? ✅
- 実依存関係を使用? ✅ 実llama.cppモデル
- Integration testの対象: 新パラメータ、usage計算、logprobs
- 禁止: テスト前の実装、REDフェーズのスキップ ✅

**可観測性**:

- 構造化ロギング含む? ✅ 既存のログ機構を使用
- エラーコンテキスト十分? ✅ OpenAI互換エラー形式

**バージョニング**:

- バージョン番号割り当て済み? ✅ semantic-release
- 変更ごとにBUILDインクリメント? ✅ 自動
- 破壊的変更を処理? ✅ 後方互換性維持

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-24157000/
├── spec.md              # 機能仕様
├── plan.md              # このファイル
├── research.md          # Phase 0 出力
├── data-model.md        # Phase 1 出力
├── quickstart.md        # Phase 1 出力
├── contracts/           # Phase 1 出力
└── tasks.md             # Phase 2 出力 (/speckit.tasks)
```

### ソースコード (変更対象)

```text
node/
├── src/api/
│   └── openai_endpoints.cpp    # 主要変更対象
├── src/inference/
│   ├── inference_params.h      # ペナルティパラメータ追加
│   └── llama_engine.cpp        # logits取得、トークン計算
├── include/api/
│   └── openai_endpoints.h      # 新関数宣言
└── tests/
    ├── contract/
    │   └── openai_api_test.cpp # 契約テスト追加
    └── integration/
        └── openai_endpoints_test.cpp # 統合テスト追加
```

## Phase 0: アウトライン＆リサーチ

### リサーチタスク

1. **llama_tokenize API調査**: トークン数計算の精度と性能
2. **llama_get_logits API調査**: logprobs計算に必要なAPI
3. **UUID生成ライブラリ調査**: C++での一意ID生成方法
4. **OpenAI API仕様確認**: 各パラメータの正確な範囲と動作

### 技術的発見

**llama.cpp トークナイザーAPI**:

```cpp
// トークン化（計数用）
int32_t llama_tokenize(
    const struct llama_model * model,
    const char * text,
    int32_t text_len,
    llama_token * tokens,
    int32_t n_max_tokens,
    bool add_bos,
    bool special
);
```

**llama.cpp logits API**:

```cpp
// 全語彙に対するlogitsを取得
float * llama_get_logits(struct llama_context * ctx);
float * llama_get_logits_ith(struct llama_context * ctx, int32_t i);
```

**出力**: [research.md](./research.md)

## Phase 1: 設計＆契約

### 1. データモデル拡張

**InferenceParams拡張**:

```cpp
struct InferenceParams {
    // 既存フィールド...
    float presence_penalty = 0.0f;   // -2.0 ~ 2.0
    float frequency_penalty = 0.0f;  // -2.0 ~ 2.0
    int n = 1;                       // 1 ~ 8 (上限)
};
```

**TokenUsage構造体**:

```cpp
struct TokenUsage {
    int prompt_tokens;
    int completion_tokens;
    int total_tokens;
};
```

**LogprobInfo構造体**:

```cpp
struct LogprobInfo {
    std::string token;
    float logprob;
    std::vector<std::pair<std::string, float>> top_logprobs;
};
```

### 2. API契約変更

**レスポンス形式拡張**:

```json
{
  "id": "chatcmpl-{uuid}",
  "created": 1704067200,
  "choices": [...],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 20,
    "total_tokens": 30
  }
}
```

### 3. 新関数

```cpp
// トークン計数
int count_tokens(const llama_model* model, const std::string& text);

// 一意ID生成
std::string generate_response_id(const std::string& prefix);

// logprobs計算
std::vector<LogprobInfo> compute_logprobs(
    llama_context* ctx,
    const std::vector<llama_token>& tokens,
    int top_logprobs
);

// ペナルティ適用
void apply_penalties(
    llama_context* ctx,
    float presence_penalty,
    float frequency_penalty,
    const std::vector<llama_token>& generated_tokens
);
```

**出力**: [data-model.md](./data-model.md), [contracts/](./contracts/)

## Phase 2: タスク計画アプローチ

**タスク生成戦略**:

- P1項目（usage, ID）を最優先
- P2項目（penalty, logprobs）を次に
- P3項目（n パラメータ）を最後に
- 各項目: テスト作成 → 実装 → 検証の順

**順序戦略**:

1. 契約テスト作成（全項目）[P]
2. usage計算実装
3. ID生成実装
4. penalty パラメータ実装
5. logprobs実装
6. n パラメータ実装
7. 統合テスト・検証

**推定タスク数**: 20-25個

## 複雑さトラッキング

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| なし | - | - |

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了（アプローチ記述）
- [ ] Phase 3: Tasks生成済み (/speckit.tasks)
- [ ] Phase 4: 実装完了
- [ ] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み（なし）

---
*憲章 v2.0.0 に基づく - `.specify/memory/constitution.md` 参照*
