# 機能仕様書: ONNX LLM推論（chat_template → tokenizer → KV-cache → decode → stream）

**機能ID**: `SPEC-fba3c39d`  
**作成日**: 2025-12-18  
**ステータス**: 実装中（PoC完了条件の確定）  

## 目的

`llm-node` が **スタブ応答ではなく**、登録された **実ONNX LLM** を用いてテキスト生成を行い、OpenAI互換の `/v1/chat/completions` を **ストリーミング含めて**成立させる。

本SPECは、PoC（Session生成だけ）から「実推論」へ移行するための最小の確定仕様を定める。

## ユーザーシナリオ＆テスト *(必須)*

### ユーザーストーリー1 - ダッシュボード登録→実モデルの応答が返る (P0)
運用者として、ダッシュボードからHFモデルを登録し、PlaygroundのChatでメッセージ送信すると **実モデルの出力**が返って欲しい（スタブ不可）。

**独立テスト**:  
1) ダッシュボードからテキスト生成モデルを登録  
2) Playgroundでモデル選択→メッセージ送信  
3) アシスタント返答が空でないこと（スタブ固定文言ではないこと）

### ユーザーストーリー2 - chat_template が適用される (P0)
運用者として、モデルに紐づく `chat_template` に従ってプロンプトが構築されることを期待する（GGUFの埋め込み相当を外部ファイルで実現）。

**独立テスト**:  
`chat_template.jinja` が存在するモデルで、`<|start|>` / `<|channel|>` 等の制御トークンを含む生成が成立し、最終出力から制御トークンが除去された `assistant` content が返る。

### ユーザーストーリー3 - ストリーミングが段階的に届く (P0)
クライアントとして、`stream: true` のときに SSE で増分が届き、最後に `data: [DONE]` が届くことを期待する。

**独立テスト**:  
`/v1/chat/completions` に `stream:true` で投げると、複数回 `data:` イベントが届き、最後に `[DONE]` で終了する。

## 要件 *(必須)*

### 機能要件

- **FR-001**: `llm-node` は `/v1/chat/completions` に対して **実ONNX推論**を行い、スタブ応答を返さない（※テスト専用のスタブモードは例外）。
- **FR-002**: プロンプト構築は次の優先順で `chat_template` を適用する:
  1) `<model_dir>/chat_template.jinja` が存在する場合はそれを使用  
  2) `<model_dir>/metadata.json` に `chat_template` があればそれを使用  
  3) いずれも無ければ既存の簡易プロンプト（互換）にフォールバック
- **FR-003**: tokenizer は `<model_dir>/tokenizer.json` をロードして使用し、prompt の encode と生成tokenの decode を行う。
- **FR-004**: 生成は **KV-cache** を用いたループで行う（prefill→decode を 1 セッション内で実現）。
- **FR-005**: `stream:true` のとき、`delta.content` を複数回送出し、最後に `data: [DONE]` を送る（OpenAI互換）。
- **FR-006**: ルーターの非ONNX→ONNX変換は、テキスト生成用途では `text-generation-with-past` を用いて **past key values を含むONNX** を生成する。

### 非機能要件

- **NFR-001**: モデル/トークナイザの欠損は 400/500 ではなく、OpenAI互換の `error` 形式で分かるエラーを返す。
- **NFR-002**: 推論は batch=1 を前提にしつつ、将来拡張を阻害しない実装（入出力名のハードコードを避ける）。
- **NFR-003**: `llm-node` のインストーラーにより配布される実行環境で動作する（Python常駐依存を追加しない）。

## 対応するモデル形式（v1）

### ONNX
- `optimum.exporters.onnx` による `--task text-generation-with-past` 出力を対象とする
- 必須入力（例）:
  - `input_ids` (int64) `[1, seq_len]`
  - `attention_mask` (int64) `[1, past_len + seq_len]`
  - `position_ids` (int64) `[1, seq_len]`
  - `past_key_values.*.(key|value)` (float/float16) `[1, n_heads, past_len, head_dim]`
- 必須出力（例）:
  - `logits` (float/float16) `[1, seq_len, vocab]`
  - `present.*.(key|value)` (float/float16) `[1, n_heads, past_len + seq_len, head_dim]`

### Tokenizer / Template
- `tokenizer.json`（Hugging Face Tokenizers 互換）
- `chat_template.jinja`（Hugging Face の chat template ファイル）

## スコープ外（v1）

- tools/function calling の実行（テンプレは render できても、tool 実行は行わない）
- 画像/音声などのマルチモーダル入力
- バッチ生成（batch>1）
- ログ確率出力、logprobs

## 成功基準 *(必須)*

1. `llm-node` の `InferenceEngine` が initialized 状態では **実ONNX生成**を行う（スタブ固定文言が返らない）。
2. `chat_template` → tokenizer → KV-cache 推論 → decode の一連が動作し、`/v1/chat/completions` が非空の content を返す。
3. `stream:true` で複数 chunk + `[DONE]` が返る。
4. `make quality-checks` が成功する。

