# audit-582 結果要約 (メトリクス・統計)
- 残1: US-005 クラウドメトリクスエクスポート (FR-034〜037/T001〜04) 全面未着手 (/api/metrics/cloud/export なし)
- 残2(実害バグ): useWebSocket.ts:7,114 が legacy NodeStatusChanged のまま。backend は EndpointStatusChanged を publish → dashboard キャッシュ無効化が不発。優先度高
- 残3: US-006 残り (T007 invitation modal テスト整理 / T008 change-password 画面カバレッジ / Model Add Wizard modal E2E)
- 残4: FR-046 ws query 認証未実装 (要否判断)
- 実装済み未チェック: T005, FR-043/044, FR-045(Wizard以外), FR-047, US-008-A/B (spec 本文未反映)
- 推奨: 3系統に子Issue化 + チェック更新
