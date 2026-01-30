# 調査: xLLM Responses API

**機能ID**: `SPEC-1a74455c`

## 概要

- Open Responses APIは`/v1/responses`で提供される
- xLLMは既存のChat Completions実装を持つため、
  入力の正規化とレスポンス形式変換で対応可能

## 既存実装の再利用

- Chat Completionsのメッセージパース
- 近似的なトークン数計算（文字数/4）
- ストリーミングSSEの配信機構

## 留意点

- tool呼び出しなどの高度機能は対象外
- 既存のエラーフォーマット（error/message）に準拠
