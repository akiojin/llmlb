# リサーチ: APIキースコープ & /api 認証

## 現行の認証経路（整理）
- **JWT**: ダッシュボードおよび管理系 API（/api/auth, /api/users, /api/api-keys など）。
- **APIキー**: OpenAI互換 API（/v1/*）。
- **ノードトークン**: /api/health（ハートビート/メトリクス、APIキー必須）。

## 対象エンドポイントの利用者
- **管理者**: /api の管理系 API を操作。
- **ノード**: /api/nodes 登録、マニフェスト取得（/api/models/registry/:model_name/manifest.json）。
- **外部クライアント**: /v1/* 推論 API。

## 仕様上の整理ポイント
- `/api` を無認証で残すことは不可 → 管理系は admin または admin のみ許可。
- ノード登録/配信は `node` スコープで統一。
- 既存 API キーは後方互換として全スコープ扱い。

## 追加方針
- エンドポイント側に `XLLM_API_KEY` を導入し、登録/配信で利用。
- スコープとユーザーロールの説明を README に集約。
