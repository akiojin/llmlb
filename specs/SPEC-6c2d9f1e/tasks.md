# タスク一覧: モデル登録キャッシュとマルチモーダルI/Oの完全動作

**機能ID**: `SPEC-6c2d9f1e`
**ステータス**: ✅ 完了

## ToDo

### 仕様・テスト（TDD）
- [x] T001 [P] ルータ統合テスト: /v1/images/* のルーティング確認（/v0/nodes へ修正、ignore解除）
- [x] T002 [P] ルータ統合テスト: /v1/audio/* のルーティング確認（/v0/nodes へ修正、ignore解除）
- [x] T003 [P] ルータ契約テスト: 0Bキャッシュがready扱いにならないこと（再ダウンロード含む）
- [x] T004 [P] ルータ契約テスト: キャッシュ有の登録は再ダウンロードなし
- [x] T005 [P] ルータ契約テスト: 削除後に /v1/models から消えること
- [x] T006 [P] ノードunit/contractテスト: register payload に supported_runtimes が含まれる
- [x] T007 [P] ノードunit/contractテスト: heartbeat payload に loaded_asr/loaded_tts/supported_runtimes が含まれる

### 実装
- [x] T010 router_model_path の健全性チェック（0B/不完全ファイルは無効）
- [x] T011 /v1/models ready 判定をサイズ基準に修正
- [x] T012 register_model のキャッシュ判定強化（0Bは再取得）
- [x] T013 NodeInfo に supported_runtimes を追加し register 送信
- [x] T014 heartbeat に loaded_asr/loaded_tts/supported_runtimes を追加
- [x] T015 main.cpp で runtime 判定と loaded_* 収集

### 検証
- [x] T020 ルータ統合/契約テストを実行
- [x] T021 ノード unit/contract テストを実行
- [x] T022 既存品質チェック一式を実行
