# 機能仕様書: xLLM Responses API対応

**機能ID**: `SPEC-1a74455c`
**作成日**: 2026-01-26
**ステータス**: 実装完了
**入力**: ユーザー説明: "xLLMでOpen Responses API（/v1/responses）に対応する"

## ユーザーシナリオ＆テスト *(必須)*

### ユーザーストーリー1 - 基本リクエスト (優先度: P1)

開発者として、xLLMに対してOpen Responses API（/v1/responses）で
テキスト生成を行いたい。これにより、Responses APIを優先する
クライアントがxLLM単体で動作できる。

**独立テスト**: /v1/responsesに`model`と`input`を送信し、
objectが`response`のJSONが返ることを確認できる。

**受け入れシナリオ**:

1. **前提** xLLMが起動している、**実行** /v1/responsesに
   `input`が文字列のリクエストを送信する、**結果**
   `output[0].content[0].text`に生成結果が含まれる

2. **前提** xLLMが起動している、**実行** /v1/responsesに
   `input`が配列のメッセージを送信する、**結果**
   最後のユーザーメッセージに対する応答が返る

---

### ユーザーストーリー2 - ストリーミング (優先度: P1)

開発者として、Responses APIのストリーミングを使って生成結果を
逐次受信したい。これにより、UIの応答性を高められる。

**独立テスト**: /v1/responsesに`stream=true`を指定し、
`text/event-stream`でイベントが返ることを確認できる。

**受け入れシナリオ**:

1. **前提** stream=trueを指定、**実行** /v1/responsesを呼び出す、
   **結果** `response.output_text.delta`のイベントが返る

2. **前提** ストリーミングが完了、**実行** 最終イベントを受信、
   **結果** `response.completed`イベントが返る

---

### ユーザーストーリー3 - 入力バリデーション (優先度: P2)

開発者として、入力が不正な場合に明確なエラーを受け取りたい。
これにより、クライアント側で原因を特定できる。

**独立テスト**: `input`や`model`が欠けたリクエストで
400エラーが返ることを確認できる。

**受け入れシナリオ**:

1. **前提** なし、**実行** `model`なしでリクエスト、
   **結果** 400エラーが返る

2. **前提** なし、**実行** `input`なしでリクエスト、
   **結果** 400エラーが返る

---

### ユーザーストーリー4 - Responses API対応フラグ (優先度: P2)

システム管理者として、xLLMがResponses APIに対応していることを
ロードバランサーから検出できるようにしたい。

**独立テスト**: /healthのレスポンスに
`supports_responses_api: true`が含まれることを確認できる。

**受け入れシナリオ**:

1. **前提** xLLMが起動している、**実行** /healthを呼び出す、
   **結果** JSONにsupports_responses_apiが含まれる

## 機能要件

- **FR-001**: xLLMは`POST /v1/responses`を提供する
- **FR-002**: `input`が文字列の場合は単一メッセージとして扱う
- **FR-003**: `input`が配列の場合はメッセージ配列として扱う
- **FR-004**: `stream=true`のときSSEでイベントを返す
- **FR-005**: `input`または`model`が欠ける場合は400エラーを返す
- **FR-006**: /healthに`supports_responses_api: true`を含める

## 非対象

- Chat Completions ↔ Responses APIの相互変換
- Toolsやtool_choiceの実行
- 外部バックエンドへのパススルー

## 成功条件

- /v1/responsesが基本リクエストとストリーミングで動作する
- contract / integration テストが追加される
- /healthにsupports_responses_apiが含まれる
