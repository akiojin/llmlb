# Tasks: パスワード管理機能（US-004）

**SPEC ID**: #580  
**生成日**: 2026-04-08  
**対象US**: US-004（パスワード管理）  
**前提**: spec.md, plan.md 完成済み

---

## Phase 0: Setup

### T-0001: 既存実装の確認（パスワード関連モジュール）

**対象**: バックエンド確認  
**アクション**:
- [ ] `llmlb/src/auth/password.rs` の既存関数を確認（hash_password, verify_password）
- [ ] `llmlb/src/api/auth.rs` の構造を確認（ハンドラー定義方式）
- [ ] `llmlb/src/db/users.rs` のDB操作メソッドを確認（update_user等）
- [ ] JWT実装、有効期限設定を確認
- [ ] `Login.tsx` の `must_change_password` リダイレクト機構を確認

**成功基準**: 既存実装の全体像が把握できたこと

---

## Phase 1: Backend Foundation（P0）

### T-0002: [TDD-RED] パスワード要件検証関数のテスト実装

**対象**: `llmlb/src/auth/password.rs`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] テストモジュール `#[cfg(test)]` を `password.rs` 追加
- [ ] `validate_password()` の失敗ケーステストを実装
  - 長さ < 8文字
  - 大文字なし
  - 数字なし
  - 複合エラー
- [ ] 成功ケーステストを実装（有効なパスワード複数パターン）

**成功基準**: テスト実行時に全テスト失敗すること（RED）

---

### T-0003: [TDD-GREEN] パスワード要件検証関数の実装

**対象**: `llmlb/src/auth/password.rs`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] `validate_password(password: &str) -> Result<(), String>` 実装
  - 最小8文字チェック
  - 大文字1以上チェック
  - 数字1以上チェック
  - エラーメッセージは段階的

**成功基準**: T-0002 のテスト全てパス

---

### T-0004: [TDD-RED] 一時パスワード生成関数のテスト実装

**対象**: `llmlb/src/auth/password.rs`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] `generate_temporary_password()` のテスト実装
  - 長さが12文字以上であること
  - 英数字のみで構成されていること
  - 毎回異なる値が生成されること

**成功基準**: テスト実行時に全テスト失敗すること（RED）

---

### T-0005: [TDD-GREEN] 一時パスワード生成関数の実装

**対象**: `llmlb/src/auth/password.rs`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] `generate_temporary_password() -> String` 実装
  - 12文字以上のランダム英数字を生成
  - 大文字、小文字、数字が含まれている

**成功基準**: T-0004 のテスト全てパス

---

### T-0006: [TDD-RED] ユーザー新規作成API（POST /api/admin/users）のテスト実装

**対象**: `llmlb/src/api/auth.rs`  
**ユーザーストーリー**: US-004 / AS-005  
**アクション**:
- [ ] 統合テストを実装
  - 成功ケース: 管理者がadmin/viewerユーザーを作成
  - エラーケース: メールアドレス重複 (400)
  - エラーケース: 認可なし (401/403)
  - メール送信エミュレーション

**成功基準**: テスト全て失敗すること（RED）

---

### T-0007: [TDD-GREEN] ユーザー新規作成API（POST /api/admin/users）の実装

**対象**: `llmlb/src/api/auth.rs`, `llmlb/src/db/users.rs`, `llmlb/src/models.rs`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] ハンドラー実装（auth.rs）
  - JWTトークンから admin ロール確認
  - リクエスト検証（メール、ユーザー名、ロール）
  - ユーザー作成（DB）
  - リセットトークン生成
  - メール送信（シミュレーション）
  - CreateUserResponse を返す
- [ ] DB メソッド実装（db/users.rs）
  - `create_user(email: &str, username: &str, role: &str) -> Result<User>`
- [ ] リクエスト・レスポンス型を models.rs に追加
  - `CreateUserRequest`, `CreateUserResponse`

**成功基準**: T-0006 のテスト全てパス

---

### T-0008: [TDD-RED] パスワード設定API（POST /api/auth/set-password）のテスト実装

**対象**: `llmlb/src/api/auth.rs`  
**ユーザーストーリー**: US-004 / AS-005  
**アクション**:
- [ ] 統合テスト実装
  - 成功ケース: 有効なトークンとパスワード
  - エラーケース: トークン無効/期限切れ (400)
  - エラーケース: パスワード要件不満足 (400)
  - エラーケース: トークン既使用 (400)

**成功基準**: テスト全て失敗すること（RED）

---

### T-0009: [TDD-GREEN] パスワード設定API（POST /api/auth/set-password）の実装

**対象**: `llmlb/src/api/auth.rs`, `llmlb/src/db/users.rs`, `llmlb/src/models.rs`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] ハンドラー実装（auth.rs）
  - トークン検証（有効期限確認）
  - パスワード要件検証
  - DB 更新
  - SetPasswordResponse を返す
- [ ] DB メソッド実装（db/users.rs）
  - `update_password_hash(user_id: i32, hash: &str) -> Result<()>`
  - `invalidate_token(token: &str) -> Result<()>`
- [ ] リクエスト・レスポンス型を models.rs に追加
  - `SetPasswordRequest`, `SetPasswordResponse`

**成功基準**: T-0008 のテスト全てパス

---

### T-0010: [TDD-RED] パスワード忘れAPI（POST /api/auth/forgot-password）のテスト実装

**対象**: `llmlb/src/api/auth.rs`  
**ユーザーストーリー**: US-004 / AS-006  
**アクション**:
- [ ] 統合テスト実装
  - 成功ケース: 登録済みメール
  - エラーケース（セキュリティ）: 未登録メール（同じレスポンス）
  - メール送信エミュレーション

**成功基準**: テスト全て失敗すること（RED）

---

### T-0011: [TDD-GREEN] パスワード忘れAPI（POST /api/auth/forgot-password）の実装

**対象**: `llmlb/src/api/auth.rs`, `llmlb/src/db/users.rs`, `llmlb/src/models.rs`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] ハンドラー実装（auth.rs）
  - メール入力受け取り
  - ユーザー存在確認（存在しなくても「送信完了」と返す）
  - リセットトークン生成（24時間有効）
  - メール送信（シミュレーション）
  - ForgotPasswordResponse を返す
- [ ] DB メソッド実装（db/users.rs）
  - `find_user_by_email(email: &str) -> Result<Option<User>>`
- [ ] リクエスト・レスポンス型を models.rs に追加
  - `ForgotPasswordRequest`, `ForgotPasswordResponse`

**成功基準**: T-0010 のテスト全てパス

---

### T-0012: [TDD-RED] パスワードリセット実行API（POST /api/auth/reset-password）のテスト実装

**対象**: `llmlb/src/api/auth.rs`  
**ユーザーストーリー**: US-004 / AS-006, AS-007  
**アクション**:
- [ ] 統合テスト実装
  - 成功ケース: 有効なトークンとパスワード
  - エラーケース: トークン無効/期限切れ (400)
  - エラーケース: パスワード要件不満足 (400)
  - エラーケース: トークン既使用 (400)

**成功基準**: テスト全て失敗すること（RED）

---

### T-0013: [TDD-GREEN] パスワードリセット実行API（POST /api/auth/reset-password）の実装

**対象**: `llmlb/src/api/auth.rs`, `llmlb/src/db/users.rs`, `llmlb/src/models.rs`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] ハンドラー実装（auth.rs）
  - トークン検証（有効期限確認）
  - パスワード要件検証
  - DB 更新
  - JWT 無効化（強制再ログイン）
  - ResetPasswordResponse を返す
- [ ] DB メソッド実装（db/users.rs）
  - `invalidate_jwt_for_user(user_id: i32) -> Result<()>` （オプション）
- [ ] リクエスト・レスポンス型を models.rs に追加
  - `ResetPasswordRequest`, `ResetPasswordResponse`

**成功基準**: T-0012 のテスト全てパス

---

### T-0014: [P] バックエンド統合・リファクタリング

**対象**: `llmlb/src/api/auth.rs`, `llmlb/src/auth/password.rs`, `llmlb/src/models.rs`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] ハンドラー間の共通ロジック抽出
- [ ] エラーハンドリングの統一
- [ ] コメント追加
- [ ] テストカバレッジ確認（80%以上）

**成功基準**: コードレビューで指摘なし

---

## Phase 2: Frontend Implementation（P1）

### T-0015: [TDD-RED] ForgotPassword.tsx コンポーネントのテスト実装

**対象**: `llmlb/src/web/dashboard/src/pages/ForgotPassword.tsx`  
**ユーザーストーリー**: US-004 / AS-006  
**アクション**:
- [ ] React Testing Library を使用したテスト実装
  - メール入力フォーム存在確認
  - 送信ボタン動作テスト
  - エラーメッセージ表示テスト
  - 成功時のメッセージ表示テスト

**成功基準**: テスト全て失敗すること（RED）

---

### T-0016: [TDD-GREEN] ForgotPassword.tsx コンポーネントの実装

**対象**: `llmlb/src/web/dashboard/src/pages/ForgotPassword.tsx`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] React コンポーネント実装
  - useState でメールアドレス状態管理
  - 送信ボタン動作
  - authApi.forgotPassword() 呼び出し
  - 成功時メッセージ表示
  - エラー時 toast 通知

**成功基準**: T-0015 のテスト全てパス

---

### T-0017: [TDD-RED] SetPassword.tsx コンポーネントのテスト実装

**対象**: `llmlb/src/web/dashboard/src/pages/SetPassword.tsx`  
**ユーザーストーリー**: US-004 / AS-005, AS-006  
**アクション**:
- [ ] React Testing Library を使用したテスト実装
  - パスワード入力フォーム存在確認
  - パスワード要件検証テスト
  - エラーメッセージ段階的表示テスト
  - 成功時のリダイレクトテスト

**成功基準**: テスト全て失敗すること（RED）

---

### T-0018: [TDD-GREEN] SetPassword.tsx コンポーネントの実装

**対象**: `llmlb/src/web/dashboard/src/pages/SetPassword.tsx`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] React コンポーネント実装
  - URL クエリからトークン抽出
  - useState でパスワード状態管理
  - パスワード要件検証（ローカル）
  - authApi.setPassword() 呼び出し
  - 成功時リダイレクト（/login）
  - エラー時 toast 通知

**成功基準**: T-0017 のテスト全てパス

---

### T-0019: [P] API呼び出し関数を lib/api.ts に追加

**対象**: `llmlb/src/web/dashboard/src/lib/api.ts`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] `authApi.forgotPassword(email: string)` 実装
  - POST `/api/auth/forgot-password`
  - エラーハンドリング
- [ ] `authApi.setPassword(token: string, password: string)` 実装
  - POST `/api/auth/set-password`
  - エラーハンドリング
- [ ] `authApi.resetPassword(token: string, password: string)` 実装
  - POST `/api/auth/reset-password`
  - エラーハンドリング
- [ ] `authApi.createUser(...)` 実装
  - POST `/api/admin/users`
  - エラーハンドリング

**成功基準**: コンポーネントから呼び出し可能

---

### T-0020: [P] Admin.tsx に「ユーザー追加」フォーム追加

**対象**: `llmlb/src/web/dashboard/src/pages/Admin.tsx`  
**ユーザーストーリー**: US-004 / AS-005  
**アクション**:
- [ ] ユーザー追加ダイアログ実装
  - メール入力
  - ユーザー名入力
  - ロール選択（admin / viewer）
  - 追加ボタン
- [ ] authApi.createUser() 呼び出し
- [ ] 成功時のユーザー一覧更新
- [ ] エラー時 toast 通知

**成功基準**: UI で実行可能

---

### T-0021: [P] HTML エントリポイント作成

**対象**: `llmlb/src/web/static/forgot-password.html`, `set-password.html`  
**ユーザーストーリー**: US-004  
**アクション**:
- [ ] `forgot-password.html` 作成（SPA エントリポイント）
  - `<div id="root"></div>` でForgotPasswordコンポーネント マウント
- [ ] `set-password.html` 作成（SPA エントリポイント）
  - `<div id="root"></div>` でSetPasswordコンポーネント マウント
- [ ] ダッシュボードビルド実行
  - `pnpm --filter @llm/dashboard build`
  - 生成物を `llmlb/src/web/static/` に確認

**成功基準**: ブラウザでアクセス可能

---

## Phase 3: Integration & E2E Tests（P0）

### T-0022: [TDD] シナリオテスト: 管理者がユーザー作成 → パスワード設定

**対象**: E2E テスト  
**ユーザーストーリー**: US-004 / AS-005  
**アクション**:
- [ ] 管理者が POST /api/admin/users でユーザー作成
- [ ] メール送信をエミュレーション（初期パスワード設定リンク）
- [ ] ユーザーが set-password.html でパスワード設定
- [ ] ユーザーがログイン可能か確認

**成功基準**: E2E テストパス

---

### T-0023: [TDD] シナリオテスト: ユーザーパスワード忘れ → リセット → ログイン

**対象**: E2E テスト  
**ユーザーストーリー**: US-004 / AS-006  
**アクション**:
- [ ] ユーザーが POST /api/auth/forgot-password で忘れ申告
- [ ] メール送信をエミュレーション（パスワードリセットリンク）
- [ ] ユーザーが forgot-password.html でリセット実行
- [ ] ユーザーがログイン可能か確認

**成功基準**: E2E テストパス

---

### T-0024: [TDD] シナリオテスト: 最後の管理者がパスワード忘れ対応

**対象**: E2E テスト  
**ユーザーストーリー**: US-004 / AS-007  
**アクション**:
- [ ] 管理者が1人のみの状態を構築
- [ ] その管理者が POST /api/auth/forgot-password → リセット → ログイン
- [ ] 成功することを確認（削除防止の制限はなし）

**成功基準**: E2E テストパス

---

### T-0025: [TDD] API テスト: トークン有効期限・一度のみ使用

**対象**: API 統合テスト  
**ユーザーストーリー**: US-004 / SC-004  
**アクション**:
- [ ] トークン期限切れで POST /api/auth/set-password → 400エラー
- [ ] トークンを2度使用 → 2回目は400エラー
- [ ] パスワード要件各エラーを検証（SC-006）

**成功基準**: テストパス

---

## Phase 4: UI Integration（P2）

### T-0026: [P] ログイン画面に「パスワード忘れ」リンク追加

**対象**: `llmlb/src/web/dashboard/src/pages/Login.tsx`  
**ユーザーストーリー**: US-004 / AS-006  
**アクション**:
- [ ] ログイン画面下部に「パスワードを忘れました」リンク追加
- [ ] クリックで `/dashboard/forgot-password.html` へ遷移

**成功基準**: UI で動作確認

---

### T-0027: [P] Admin 画面に「ユーザー追加」ボタン実装

**対象**: `llmlb/src/web/dashboard/src/pages/Admin.tsx`  
**ユーザーストーリー**: US-004 / AS-005  
**アクション**:
- [ ] ユーザー一覧上部に「ユーザー追加」ボタン
- [ ] ボタンクリックでダイアログ表示（T-0020）
- [ ] 追加後、一覧を自動更新

**成功基準**: UI で動作確認

---

## Phase 5: Polish & Documentation（P2）

### T-0028: パスワード管理関連のログ追加

**対象**: `llmlb/src/api/auth.rs`  
**アクション**:
- [ ] ユーザー作成時のログ（admin がユーザー作成）
- [ ] パスワード設定時のログ（トークン検証結果）
- [ ] パスワードリセット時のログ（メール送信）
- [ ] ログレベル: info（正常）, warn（トークン期限切れ等）, error（重大）

**成功基準**: ログ出力確認

---

### T-0029: API ドキュメント更新

**対象**: `docs/api.md` 等  
**アクション**:
- [ ] POST /api/admin/users ドキュメント追加
- [ ] POST /api/auth/forgot-password ドキュメント追加
- [ ] POST /api/auth/set-password ドキュメント追加
- [ ] POST /api/auth/reset-password ドキュメント追加
- [ ] リクエスト・レスポンス例を記載

**成功基準**: ドキュメント完成

---

### T-0030: CHANGELOG.md 更新

**対象**: `CHANGELOG.md`  
**アクション**:
- [ ] US-004 をリリースノートに記載
- [ ] 新規エンドポイント4つをリスト
- [ ] パスワード要件を明記

**成功基準**: リリースノート完成

---

## 成功基準の追跡

| 成功基準 | 対応タスク |
|---------|-----------|
| SC-001 | T-0006, T-0007, T-0020, T-0022 |
| SC-002 | T-0006, T-0007, T-0016, T-0018, T-0022 |
| SC-003 | T-0010, T-0011, T-0015, T-0016, T-0023 |
| SC-004 | T-0012, T-0013, T-0025 |
| SC-005 | T-0023, T-0024 |
| SC-006 | T-0002, T-0003, T-0017, T-0018, T-0025 |
| SC-007 | T-0012, T-0013 |
