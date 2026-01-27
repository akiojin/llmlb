# xLLM 統合仕様 - タスク一覧

## Setup

- [x] S001: SPECディレクトリ作成
- [x] S002: spec.md作成
- [x] S003: plan.md作成
- [x] S004: tasks.md作成

## Cleanup（優先）

- [x] C001: router_endpoints.cpp 削除
- [x] C002: router_models.cpp 削除
- [x] C003: router_status.cpp 削除
- [x] C004: 関連するヘッダ・CMakeLists更新

## Phase 1: サーバーモード完成

### Test

- [x] T101: serve コマンドの契約テスト作成（cli_serve_test.cpp）
- [x] T102: config.json読み込みのunitテスト作成（utils_config_test.cpp, YAML→JSON変更）
- [x] T103: シグナルハンドリングのテスト作成（cli_server_test.cpp, DISABLED）

### Core

- [x] T104: serve コマンド実装（main.cppでrun_node呼び出し）
- [x] T105: config.json パーサー実装（~/.xllm/config.json対応）
- [x] T106: シグナルハンドリング実装（SIGINT/SIGTERM, main.cpp）
- [x] T107: --port, --host オプション対応（cli.cpp）

### Integration

- [x] T108: serve起動→API呼び出し→終了の統合テスト（http_server_test.cpp）

## Phase 2: メモリ管理強化

### Test

- [ ] T201: LRUアンロードのunitテスト作成
- [ ] T202: アクティブ保護のテスト作成
- [ ] T203: Tensor Parallelism自動化のテスト作成
- [ ] T210: アイドルタイムアウトのテスト作成
- [ ] T211: GPU Offloadのテスト作成

### Core

- [ ] T204: VRAM使用量監視機能
- [ ] T205: LRUアンロードロジック実装
- [ ] T206: アクティブモデル保護ロジック
- [ ] T207: [P] GPU検出・VRAM計算
- [ ] T208: [P] split比率自動決定
- [ ] T212: アイドルタイムアウト実装（config.yamlで設定可能）
- [ ] T213: GPU Offload実装（n_gpu_layers自動調整）

### Integration

- [ ] T209: 複数モデルロード→VRAM枯渇→LRUアンロードの統合テスト
- [ ] T214: アイドル→自動アンロードの統合テスト

## Phase 3: マルチモーダル強化

### Test

- [ ] T301: TTSストリーミングの契約テスト作成
- [ ] T302: 画像URL配信のテスト作成
- [ ] T303: 画像TTL削除のテスト作成

### Core

- [ ] T304: TTSチャンク生成実装
- [ ] T305: TTSストリーミングレスポンス実装
- [ ] T306: [P] 画像ファイル保存実装
- [ ] T307: [P] 静的ファイル配信エンドポイント
- [ ] T308: 画像TTL自動削除（バックグラウンドタスク）

### Integration

- [ ] T309: TTS→ストリーミング受信の統合テスト
- [ ] T310: 画像生成→URL取得→ファイル確認の統合テスト

## Phase 4: 高度な推論機能

### Test

- [ ] T401: Function Calling契約テスト作成
- [ ] T402: Speculative Decodingテスト作成
- [ ] T409: リクエストキューイングテスト作成

### Core

- [ ] T403: grammar/constrained decoding統合
- [ ] T404: ツール出力形式の強制ロジック
- [ ] T405: [P] ドラフトモデル管理
- [ ] T406: [P] 投機的デコード検証ロジック
- [ ] T410: リクエストキューイング実装（タイムアウト付き）

### Integration

- [ ] T407: Function Calling E2Eテスト
- [ ] T408: Speculative Decoding性能テスト
- [ ] T411: バッチ超過→キュー待機→処理の統合テスト

## Phase 5: 運用機能

### Test

- [ ] T501: Prometheus Metrics形式テスト
- [ ] T502: Modelfileパーサーテスト

### Core

- [ ] T503: /metrics エンドポイント実装
- [ ] T504: トークンスループットメトリクス
- [ ] T505: VRAM使用率メトリクス
- [ ] T506: [P] Modelfileパーサー実装
- [ ] T507: [P] Modelfile全命令対応

### Integration

- [ ] T508: Prometheus scrape統合テスト
- [ ] T509: Modelfile適用→推論の統合テスト

## Phase 6: 追加CLI機能

### Test

- [ ] T601: profile コマンドの契約テスト作成
- [ ] T602: benchmark コマンドの契約テスト作成
- [ ] T603: compare コマンドの契約テスト作成
- [ ] T604: convert コマンドの契約テスト作成
- [ ] T605: export/import コマンドの契約テスト作成

### Core

- [ ] T606: [P] profile コマンド実装（VRAM/速度計測）
- [ ] T607: [P] benchmark コマンド実装（トークン/秒）
- [ ] T608: compare コマンド実装（モデル比較）
- [ ] T609: convert コマンド実装（形式変換）
- [ ] T610: [P] export コマンド実装
- [ ] T611: [P] import コマンド実装

## Phase 7: 高度な機能

### Test

- [ ] T701: Dynamic LoRAのunitテスト作成
- [ ] T702: KVキャッシュ永続化のテスト作成
- [ ] T703: モデルレプリカのテスト作成

### Core

- [ ] T704: Dynamic LoRA実装（実行時ロード）
- [ ] T705: KVキャッシュディスク永続化実装
- [ ] T706: モデルレプリカ実装（同一モデル複数インスタンス）

### Integration

- [ ] T707: LoRAロード→推論の統合テスト
- [ ] T708: KVキャッシュ永続化→復元の統合テスト

## Phase 8: HTTP機能強化

### Test

- [ ] T801: CORS設定のテスト作成
- [ ] T802: Gzip圧縮のテスト作成
- [ ] T803: X-Request-IDのテスト作成
- [ ] T804: ログローテーションのテスト作成

### Core

- [ ] T805: [P] CORS設定実装（config.yaml対応）
- [ ] T806: [P] Gzip圧縮実装
- [ ] T807: X-Request-IDヘッダー実装
- [ ] T808: ログローテーション実装

### Integration

- [ ] T809: CORS→クロスオリジンリクエストの統合テスト

## Polish

- [ ] P001: ドキュメント更新（README.md）
- [ ] P002: CLIヘルプメッセージ整備
- [ ] P003: エラーメッセージ統一
- [ ] P004: PGP署名検証実装
- [ ] P005: メタデータキャッシュ実装

## 優先度マトリクス

| タスク群 | 優先度 | 依存 |
|----------|--------|------|
| Cleanup | 高 | なし |
| Phase 1 | 高 | Cleanup |
| Phase 2 | 高 | Phase 1 |
| Phase 3 | 中 | Phase 1 |
| Phase 4 | 中 | Phase 2 |
| Phase 5 | 低 | Phase 4 |
| Phase 6 | 低 | Phase 1 |
| Phase 7 | 低 | Phase 4 |
| Phase 8 | 低 | Phase 1 |

## 並列実行可能タスク

`[P]` マークのタスクは他のタスクと並列実行可能:

- T104, T105（serve + config）
- T207, T208（GPU検出 + split計算）
- T306, T307（画像保存 + 配信）
- T405, T406（ドラフトモデル + 検証）
- T506, T507（Modelfileパーサー + 命令対応）
- T606, T607（profile + benchmark）
- T610, T611（export + import）
- T805, T806（CORS + Gzip）
