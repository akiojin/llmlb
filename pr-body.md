## Summary

- ストリーミングリクエストに `stream_options.include_usage: true` を注入し、Ollama などのエンドポイントが最終 SSE チャンクに usage を含めるようにした
- これにより `output_tokens > 0` が満たされ、TPS が正常に記録・更新される
- ダッシュボードのエンドポイント詳細画面でモデル別 TPS が表示されるようになる

## Changes

- `llmlb/src/api/openai.rs`: ストリーミングリクエストを upstream へ転送する際、`stream_options.include_usage` を注入するロジックを追加（行 981-992）

## Testing

- [x] `cargo fmt --check` — OK（フォーマットエラーなし）
- [x] `cargo clippy -- -D warnings` — OK（警告なし）
- [x] `cargo test` — OK（128 tests passed）
- [x] `pnpm dlx markdownlint-cli2` — OK（0 errors）

## Closing Issues

- None

## Related Issues / Links

- SPEC-8c32349f （Phase 7: エンドポイント単位リクエスト統計可視化）

## Checklist

- [x] Tests added/updated — N/A: 既存テストがすべて pass（新規テストは後続タスク）
- [x] Lint/format passed (`cargo clippy`, `cargo fmt`, `svelte-check`)
- [x] Documentation updated — N/A: 内部実装改善、ユーザー向け変更なし
- [x] Migration/backfill plan included — N/A: スキーマ変更なし
- [x] CHANGELOG impact considered — バグ修正（TPS 算出機能の修正）

## Context

Ollama のような OpenAI 互換 API がストリーミング時にデフォルトでは `usage` フィールドを返さない仕様により、
`StreamingTokenAccumulator` が `extracted_usage` を取得できず、`output_tokens = 0` となってしまい、
TPS 更新条件 `should_update_tps = false` を招いていた。

クライアントやバックエンド側で `stream_options: {"include_usage": true}` を指定することで、
Ollama や他の OpenAI 互換エンドポイントが最終チャンクに usage を含める。

このアプローチは OpenAI API の仕様に準拠し、すべての OpenAI 互換エンドポイントで機能する。

## Risk / Impact

- **Affected areas**: ストリーミングリクエストの upstream ペイロード構築部分（`llmlb/src/api/openai.rs`）
- **Compatibility**: `stream_options` をサポートしないエンドポイントは無視するため、副作用なし
- **Rollback plan**: 当該コード変更（行 983-992）を削除し、redeploy（git revert も可）

## Notes

- クライアントが `stream_options` を既に指定している場合は `or_insert` により既存値を優先（上書きしない）
- 非ストリーミング（`stream: false`）はこの修正の影響を受けない（既に Ollama が usage を返す）
- ダッシュボードのエンドポイント詳細画面でモデル別 TPS 表（EndpointModelTpsTable）が正常に機能するようになる

🤖 Generated with Claude Code
