# 機能仕様書: API統合リファレンス

**機能ID**: `SPEC-82cd11b7`
**作成日**: 2026-02-19
**ステータス**: 下書き
**入力**: ユーザー説明: "API統合リファレンス - 全APIエンドポイントのカタログを一元的に参照可能にする仕様書。認証モデル分類、API設計規約、関連SPECへの相互参照を含む。新規要件としてGET /api/version（認証不要）を追加する。"

## ユーザーシナリオ＆テスト *(必須)*

### ユーザーストーリー1 - リモートサーバーのバージョン確認 (優先度: P1)

運用者として、llmlbサーバーのバージョンを認証なしで取得したい。
リモートサーバーの稼働バージョンを確認できないと、更新判断やトラブルシュート時に
ダッシュボードへのログインが必要になり、運用効率が低下するため。

**この優先度の理由**: バージョン確認は運用診断の最も基本的な操作であり、
監視システムやスクリプトからの自動取得にも必要不可欠。認証不要にすることで
ヘルスチェックと同様に外部から気軽に利用できる。

**独立テスト**: `GET /api/version` を認証ヘッダーなしで呼び出し、
バージョン文字列を含むJSONレスポンスが返ることを確認することで、
独立して価値を提供する。

**受け入れシナリオ**:

1. **前提** llmlbサーバーが起動している、**実行** 認証なしで `GET /api/version` を呼び出す、**結果** HTTP 200 で `{"version":"X.Y.Z"}` 形式のJSONが返る
2. **前提** llmlbサーバーが起動している、**実行** APIキー付きで `GET /api/version` を呼び出す、**結果** 認証ヘッダーは無視され、同様にHTTP 200が返る（認証不要のため）
3. **前提** llmlbのバージョンが `4.7.0`、**実行** `GET /api/version` を呼び出す、**結果** `{"version":"4.7.0"}` が返る（Cargo.tomlのバージョンと一致）

---

### ユーザーストーリー2 - 全APIエンドポイントの一覧把握 (優先度: P2)

開発者として、llmlbが提供する全APIエンドポイントを一覧で把握したい。
APIエンドポイントの定義が10以上のSPECに分散しており、新しいAPI利用者や
連携システムの開発者が全体像を把握するのに時間がかかるため。

**この優先度の理由**: APIカタログは開発者のオンボーディングを加速し、
重複したエンドポイントの作成を防ぐ。ドキュメントとしての価値が高い。

**独立テスト**: 本仕様書のエンドポイントカタログセクションを参照し、
各エンドポイントのパス・認証方式・関連SPECが記載されていることを
確認することで、独立して価値を提供する。

**受け入れシナリオ**:

1. **前提** 開発者がllmlbのAPIを利用したい、**実行** 本仕様書を参照する、**結果** 全エンドポイントのパス、HTTPメソッド、認証方式、関連SPECが確認できる
2. **前提** 新しいAPIエンドポイントを追加する開発者、**実行** 本仕様書のカタログを参照する、**結果** 既存エンドポイントとの命名規則の一貫性を確認できる

---

### ユーザーストーリー3 - 認証方式の判断 (優先度: P2)

API利用者として、各エンドポイントに必要な認証方式を事前に知りたい。
エンドポイントごとにJWT認証、APIキー認証、認証不要のいずれかが異なるため、
適切なヘッダーを設定しないとアクセスが拒否される。

**この優先度の理由**: 認証方式の誤りは最も頻繁に発生するAPI利用エラーであり、
明確な分類があれば試行錯誤を減らせる。

**独立テスト**: 本仕様書の認証モデル分類を参照し、各エンドポイントカテゴリの
認証方式が明記されていることを確認することで、独立して価値を提供する。

**受け入れシナリオ**:

1. **前提** API利用者がダッシュボード操作を自動化したい、**実行** 本仕様書の認証分類を参照する、**結果** ダッシュボードAPIはJWT認証が必要と判断できる
2. **前提** API利用者が推論APIを利用したい、**実行** 本仕様書の認証分類を参照する、**結果** OpenAI互換APIはAPIキー認証が必要と判断できる
3. **前提** 監視スクリプトがバージョンを取得したい、**実行** 本仕様書の認証分類を参照する、**結果** `/api/version` は認証不要と判断できる

---

### エッジケース

- バージョン文字列がセマンティックバージョニング（`X.Y.Z`）に準拠していない場合でも、Cargo.tomlの値をそのまま返す
- サーバー起動直後（初期化完了前）でもバージョンAPIは応答可能であること

## 要件 *(必須)*

### 機能要件

- **FR-001**: システムは `GET /api/version` エンドポイントを提供し、認証なしでアクセスできる
- **FR-002**: バージョンAPIは `{"version":"X.Y.Z"}` 形式のJSONレスポンスを返す（Content-Type: application/json）
- **FR-003**: バージョン値はビルド時のCargo.tomlに定義されたバージョンと一致する
- **FR-004**: 本仕様書は全APIエンドポイントのカタログとして、パス、HTTPメソッド、認証方式、関連SPECを一覧化する
- **FR-005**: 各エンドポイントの認証方式は JWT / APIKey / None の3種類に分類される

### エンドポイントカタログ

| カテゴリ | パスプレフィックス | 認証 | 関連SPEC |
|---------|-------------------|------|---------|
| OpenAI互換推論 | `/v1/chat/completions`, `/v1/completions`, `/v1/embeddings` | APIKey | SPEC-0f1de549 |
| Open Responses | `/v1/responses` | APIKey | SPEC-0f1de549 |
| Audio API | `/v1/audio/transcriptions`, `/v1/audio/speech` | APIKey | SPEC-617247d2 |
| Image API | `/v1/images/generations`, `/v1/images/edits`, `/v1/images/variations` | APIKey | SPEC-5fc9fe92 |
| モデル一覧（外部） | `/v1/models`, `/v1/models/{model_id}` | APIKey | SPEC-0f1de549 |
| モデル一覧（内部） | `/api/models`, `/api/models/hub` | JWT/APIKey | SPEC-6cd7f960 |
| モデル管理 | `/api/models/register`, `/api/models/{model_name}` | JWT/APIKey (Admin) | SPEC-6cd7f960 |
| モデル配布 | `/api/models/registry/{model_name}/manifest.json` | APIKey | SPEC-e8e9326e |
| エンドポイント参照 | `/api/endpoints`, `/api/endpoints/{id}`, `/api/endpoints/{id}/models` | JWT/APIKey | SPEC-e8e9326e |
| エンドポイント管理 | `/api/endpoints` (POST), `/api/endpoints/{id}` (PUT/DELETE) | JWT/APIKey (Admin) | SPEC-e8e9326e |
| エンドポイントテスト | `/api/endpoints/{id}/test`, `/api/endpoints/{id}/sync` | JWT/APIKey (Admin) | SPEC-e8e9326e |
| エンドポイントダウンロード | `/api/endpoints/{id}/download`, `/api/endpoints/{id}/download/progress` | JWT/APIKey (Admin) / JWT/APIKey | SPEC-e8e9326e |
| モデル情報 | `/api/endpoints/{id}/models/{model}/info` | JWT/APIKey | SPEC-e8e9326e |
| Playground プロキシ | `/api/endpoints/{id}/chat/completions` | JWT | SPEC-e8e9326e |
| エンドポイント統計 | `/api/endpoints/{id}/today-stats`, `/api/endpoints/{id}/daily-stats`, `/api/endpoints/{id}/model-stats` | JWT/APIKey | SPEC-8c32349f |
| TPS可視化 | `/api/endpoints/{id}/model-tps` | JWT/APIKey | SPEC-4bb5b55f |
| ダッシュボード | `/api/dashboard/endpoints`, `/api/dashboard/models`, `/api/dashboard/stats` 等 | JWT | SPEC-712c20cf |
| ダッシュボード統計 | `/api/dashboard/stats/tokens`, `/api/dashboard/stats/tokens/daily`, `/api/dashboard/stats/tokens/monthly` | JWT | SPEC-712c20cf |
| ダッシュボードログ | `/api/dashboard/logs/lb` | JWT | SPEC-712c20cf |
| モデル別統計 | `/api/dashboard/model-stats` | JWT | SPEC-712c20cf |
| リクエスト履歴 | `/api/dashboard/request-responses`, `/api/dashboard/request-responses/{id}`, `/api/dashboard/request-responses/export` | JWT | SPEC-fbc50d97 |
| 認証 | `/api/auth/login` (POST), `/api/auth/register` (POST) | None | SPEC-d4eb8796 |
| 認証（要ログイン） | `/api/auth/me` (GET), `/api/auth/logout` (POST) | JWT | SPEC-d4eb8796 |
| ユーザー管理 | `/api/users`, `/api/users/{id}` | JWT/APIKey (Admin) | SPEC-d4eb8796 |
| APIキー管理 | `/api/api-keys`, `/api/api-keys/{id}` | JWT/APIKey (Admin) | SPEC-d4eb8796 |
| 招待管理 | `/api/invitations`, `/api/invitations/{id}` | JWT/APIKey (Admin) | SPEC-d4eb8796 |
| ノードログ | `/api/nodes/{node_id}/logs` | JWT/APIKey (Admin) | SPEC-712c20cf |
| メトリクス | `/api/metrics/cloud` | JWT/APIKey (Admin) | SPEC-712c20cf |
| システム情報 | `/api/system` | JWT | SPEC-a6e55b37 |
| システム更新 | `/api/system/update/check` (POST), `/api/system/update/apply` (POST) | JWT (Admin) | SPEC-a6e55b37 |
| バージョン（新規） | `/api/version` | None | 本SPEC |
| WebSocket | `/ws/dashboard` | JWT | SPEC-712c20cf |
| ダッシュボードUI | `/dashboard`, `/dashboard/*` | None（静的アセット） | SPEC-712c20cf |

### 認証モデル分類

| 認証方式 | 説明 | 対象 |
|---------|------|------|
| None | 認証不要 | `/api/auth/login`, `/api/auth/register`, `/api/version`, ダッシュボード静的アセット |
| APIKey | APIキーによる認証（`Authorization: Bearer sk_xxx`） | OpenAI互換推論API、Audio/Image API、`/v1/models` |
| JWT | JWTトークンによる認証（ダッシュボードログインで取得） | ダッシュボードAPI、システムAPI |
| JWT/APIKey | JWTまたはAPIキーのいずれかで認証可能 | エンドポイント管理、ユーザー管理、統計API等 |

### API設計規約

- パス命名: OpenAI互換は `/v1/` プレフィックス、llmlb独自APIは `/api/` プレフィックス
- レスポンス形式: すべてJSON（Content-Type: application/json）
- エラー形式: OpenAI互換APIはOpenAIエラー形式、`/api/*` はllmlb独自エラー形式
- HTTPメソッド: CRUD操作は REST 規約に準拠（GET=参照、POST=作成、PUT=更新、DELETE=削除）

### 関連仕様

| SPEC ID | 名称 | 管理するエンドポイント |
|---------|------|---------------------|
| SPEC-0f1de549 | OpenAI互換API | `/v1/chat/completions`, `/v1/completions`, `/v1/embeddings`, `/v1/responses`, `/v1/models` |
| SPEC-617247d2 | Audio API | `/v1/audio/*` |
| SPEC-5fc9fe92 | Image API | `/v1/images/*` |
| SPEC-6cd7f960 | モデルHub | `/api/models/hub` |
| SPEC-e8e9326e | エンドポイント管理 | `/api/endpoints/*` |
| SPEC-d4eb8796 | 認証・アクセス制御 | `/api/auth/*`, `/api/users/*`, `/api/api-keys/*`, `/api/invitations/*` |
| SPEC-712c20cf | 管理ダッシュボード | `/api/dashboard/*`, `/ws/dashboard` |
| SPEC-a6e55b37 | 自動アップデート | `/api/system`, `/api/system/update/*` |
| SPEC-8c32349f | エンドポイント統計 | `/api/endpoints/{id}/*-stats` |
| SPEC-4bb5b55f | TPS可視化 | `/api/endpoints/{id}/model-tps` |
| SPEC-fbc50d97 | リクエスト履歴 | `/api/dashboard/request-responses*` |

## 成功基準 *(必須)*

### 測定可能な結果

- **SC-001**: `GET /api/version` が認証なしで200レスポンスを返し、正しいバージョン文字列を含む
- **SC-002**: 本仕様書がすべてのAPIエンドポイント（90+）をカタログ化し、認証方式と関連SPECが記載されている
- **SC-003**: 関連する10件のSPECから本仕様書への相互参照が設定されている

## スコープ外 *(オプション)*

- OpenAPI/Swagger定義の自動生成（将来拡張として検討可能）
- APIバージョニング戦略の変更（現状の `/v1/` と `/api/` の二層構造は維持）
- レート制限やAPIクォータの仕様定義（別SPECで管理）
