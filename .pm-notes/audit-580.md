# audit-580 結果要約
- 残: US-004/US-008 Eメール形式パスワード管理が丸ごと未実装
  - forgot-password/reset token 発行・検証 (AS-014/015/016, T002/T003前半): ルート/ハンドラ/テスト/マイグレーションなし
  - AS-017: change-password に current_password 検証なし (auth.rs:79)
  - T001 email identifier contract: users.rs create_user にバリデーションなし
  - T005 forgot/reset UI なし
  - T006 invitation 系 canonical scope 除去が未実施 (spec 内で矛盾あり)
- 実装済み未チェック: US-013 認証必須化 (tdd 3項目, acceptance 4項目, T-013-01〜07) → チェック更新のみ
- 軽微掃除: playwright.config.ts の LLMLB_API_KEY_REQUIRED 残骸, lb.rs:81 コメント
- 推奨: 子Issue化(US-004/US-008 残作業) + checklist 更新 + invitations scope 矛盾の意思決定
