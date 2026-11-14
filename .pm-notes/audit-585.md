# audit-585 結果要約 (CI/CD・テスト・リリース)
- 残 (US-004 集中): FR-027 契約テスト欠落 (/audit-logs*, /benchmarks/tps, /catalog/search, /ws/chat ゼロ、/system/update* /metrics/cloud /models/hub は契約なし) / FR-032 カバレッジ80%ゲート未整備 (ci.yml は lcov 出力のみ) / FR-028 クリティカルパス定義自体なし
- 実装済み未チェック: FR-030 Playwright 50 spec, FR-029, FR-031 → tdd/acceptance のE2E項目チェック更新可
- 要判断: FR-028「クリティカルパス」定義はメンテナ判断が必要
- 推奨: チェック更新 + 子Issue化 (契約テスト追加 / カバレッジゲート / FR-028 spec明文化)
