# 実装計画: モデル登録キャッシュとマルチモーダルI/Oの完全動作

**機能ID**: `SPEC-6c2d9f1e` | **日付**: 2025-12-20 | **仕様**: [spec.md](./spec.md)
**ステータス**: 🟡 実装中（TDD）

## 概要

モデル登録キャッシュの健全性チェック、指定モデルでのチャット、画像/音声入出力のルーティング、削除反映を完結させる。ノードは supported_runtimes と音声/画像のロード状況を登録・ハートビートで送信し、ロードバランサーはそれを利用して API リクエストを適切なノードへ転送する。

## 実装方針

1. **仕様・テストを先行**（TDD）: ロードバランサー統合テストとノード unit/contract テストを追加・修正。
2. **ノード登録/ハートビート拡張**: supported_runtimes と loaded_asr/tts を送信。
3. **キャッシュ健全性**: 0B/不完全ファイルを無効化し再取得。
4. **ルーティング**: 画像/音声の RuntimeType に基づくノード選択を実利用に合わせて整備。
5. **削除反映**: router_model_path の検証強化で /v1/models の ready 判定を厳密化。

## 主要変更箇所

- `node/include/api/router_client.h`, `node/src/api/router_client.cpp`: supported_runtimes/loaded_* 送信
- `node/src/main.cpp`: runtime 判定と heartbeat ペイロード更新
- `llmlb/src/registry/models.rs`: キャッシュ健全性チェック
- `llmlb/src/api/openai.rs`: /v1/models ready 判定をサイズ基準に
- `llmlb/tests/integration/audio_api_test.rs`, `llmlb/tests/integration/images_api_test.rs`: ルーティングテスト更新
- `node/tests/unit/router_client_test.cpp`, `node/tests/contract/router_api_test.cpp`: payload検証更新

## テスト方針

- ロードバランサー統合テスト: 画像/音声のノード選択とAPIルートの存在確認
- 契約テスト: モデル登録キャッシュの健全性、削除反映
- ノード unit/contract テスト: register/heartbeat payloadの追加フィールド
- 既存の Playwright はモック依存のため本件では最小限の影響範囲のみ確認

