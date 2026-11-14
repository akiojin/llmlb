# audit-586 結果要約 (アーキテクチャ・リファクタリング)
- 残: US-009〜013 (旧#506 物理分割) が未着手。1500行超が15件 (update/mod.rs 4634, api/openai.rs 4359, db/audit_log.rs 3652, api/dashboard.rs 3531, balancer/mod.rs 2928)。AppState 13フィールドのまま、api/mod.rs create_app に .route() 89個
- #506 は Planned のまま CLOSED、後継SPECなし → 宙吊り
- 実装済み未チェック: US-011 プラットフォーム分岐集約 (cfg(target_os) 2ファイルのみ)、FR-015 (v6.1.0)
- 推奨: successor SPEC 起票して US-009/010/012/013 移管 (行数は develop 実測値で転記) → #586 は US-011 チェック更新+T004消し込みの上 umbrella close
