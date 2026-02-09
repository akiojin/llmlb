# 実装計画: APIキー権限（Permissions）& /api 認証強化

**機能ID**: `SPEC-7c0a37e0` ｜ **作成日**: 2025-12-20（2026-02-09 更新）  
**参照仕様**: [spec.md](./spec.md)

## 概要

- APIキーに `permissions` を持たせ、外部API（`/v1/*`）と運用API（`/api/*` の一部）を最小権限で分離する。
- `/api/dashboard/*` は **JWTのみ**（APIキー不可）に固定する（SPEC-d4eb8796）。
- 旧`scopes`は廃止し、DBマイグレーションで`permissions`へ移行する（互換維持）。

## 技術コンテキスト

- **対象領域**:
  - `llmlb/src/common/auth.rs`: `ApiKeyPermission` 列挙、`ApiKey.permissions`。
  - `llmlb/src/db/api_keys.rs`: `permissions` の永続化・読み取り。
  - `llmlb/src/auth/middleware.rs`: APIキー認証 + 権限チェック、JWT/JWT+APIキー併用ルートの統一。
  - `llmlb/src/api/mod.rs`: ルート毎のrequired permission設定、`/api/dashboard/*` のJWT限定。
  - `llmlb/src/api/api_keys.rs`: APIキー発行APIの `permissions` 化、`scopes` の 400。
  - `llmlb/src/web/dashboard`: 権限チェックボックスUI。
  - `llmlb/migrations/012_add_api_key_permissions.sql`: schema追加 + legacy backfill。
- **制約**:
  - 認証スキップのフラグは禁止（CLAUDE.md）。
  - ダッシュボード→バックエンドはJWTのみ（内部APIトークンは使用しない）。

## Phase 0: 仕様確認

- SPEC-d4eb8796（ダッシュボードJWT限定、内部APIトークン廃止、/v1はAPIキー必須）
- 既存 `/api` ルートの分類（dashboard専用 / 運用自動化 / 外部API）

## Phase 1: 設計

- permission ID の固定セットを定義し、公開契約として扱う。
- ルートごとに required permission を割り当てる（例: `/v1/*` は `openai.*`）。
- JWT role と API key permission の交点を明確化する:
  - READ は viewer でも可（例: `/api/endpoints`）。
  - WRITE は JWT admin または対応する `*.manage` 権限。

## Phase 2: TDD 方針

- Contract/Integration/E2E を `permissions` 前提に更新する。
- `/api/dashboard/*` がAPIキーを受け付けないことをテストで担保する。
- 旧`scopes`の互換（DB移行とAPI拒否）を仕様/テストで固定する。

## リスクと対応

- **旧クライアント破壊**: `scopes`を受け付けないため400を返し、移行ガイド/ドキュメントで誘導する。
- **権限不足で詰まる**: ルート→必要権限の対応表をREADME/ドキュメントに明記する。
- **移行ミス**: 旧`scopes`→`permissions`のマッピングをマイグレーションで固定する。
