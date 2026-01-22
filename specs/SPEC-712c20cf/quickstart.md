# クイックスタート: 管理ダッシュボード

**機能ID**: `SPEC-712c20cf` | **日付**: 2025-10-31

## 概要

管理ダッシュボードの動作を確認するためのクイックスタートガイド。spec.mdのユーザーストーリーを検証する手順を示します。

## 前提条件

- router が起動している
- 1台以上のノードが登録されている（SPEC-94621a1f実装済み）
- Webブラウザ（Chrome, Firefox, Safari, Edge）

## シナリオ1: ダッシュボードアクセス

**目的**: spec.mdの受け入れシナリオ1を検証

### ステップ
1. routerを起動
   ```bash
   cargo run -p llmlb
   ```
   期待される出力:
   ```
   [INFO] LLM Load Balancer starting on 0.0.0.0:32768
   [INFO] Dashboard available at http://localhost:32768/dashboard
   ```

2. ブラウザで`http://localhost:32768/dashboard`にアクセス

3. 期待される結果:
   - ダッシュボードページが表示される
   - ヘッダーに「LLM Load Balancer Dashboard」が表示される
   - システム統計カードが表示される（Total Nodes, Online Nodes, etc.）
   - ノード一覧テーブルが表示される

### 検証ポイント
- [ ] ページが2秒以内にロードされる（NFR-011）
- [ ] レスポンシブデザインで表示される（NFR-010）
- [ ] JavaScriptエラーがブラウザコンソールに表示されない

---

## シナリオ2: ノード情報表示

**目的**: spec.mdの受け入れシナリオ2を検証

### ステップ
1. 複数のノード（最低3台）を登録
   ```bash
   cmake -S node -B node/build
   cmake --build node/build -j

   # ターミナル1
   LLMLB_URL=http://localhost:32768 XLLM_PORT=32769 ./node/build/xllm

   # ターミナル2
   LLMLB_URL=http://localhost:32768 XLLM_PORT=11436 ./node/build/xllm

   # ターミナル3
   LLMLB_URL=http://localhost:32768 XLLM_PORT=11437 ./node/build/xllm
   ```

2. ダッシュボードでノード一覧を確認

3. 期待される結果:
   - 各ノードの情報が表示される:
     - ✅ マシン名（例: server-01）
     - ✅ IPアドレス（例: 192.168.1.100）
     - ✅ ステータス（Online/Offline）
     - ✅ LLM runtimeバージョン（例: 0.1.0）
     - ✅ 稼働時間（例: 2h 30m）

### 検証ポイント
- [ ] 全ノードが一覧に表示される
- [ ] ステータスバッジが正しく色分けされる（Online=緑、Offline=グレー）
- [ ] 稼働時間が「Xh Ym」形式で表示される
- [ ] IPアドレスが有効な形式で表示される

---

## シナリオ3: システム統計表示

**目的**: システム統計カードの動作確認

### ステップ
1. ダッシュボードページでシステム統計カードを確認

2. 期待される結果:
   - Total Nodes: 登録済みノード総数が表示
   - Online Nodes: オンラインノード数が表示
   - Offline Nodes: オフラインノード数が表示
   - Total Requests: 総リクエスト数（初期実装では0）
   - Avg Response Time: 平均レスポンスタイム（初期実装では0ms）

### 検証ポイント
- [ ] Total Nodes の値が正しい
- [ ] Online Nodes + Offline Nodes = Total Nodes
- [ ] 統計が見やすくカード形式で表示される

---

## シナリオ4: リアルタイム更新

**目的**: spec.mdの受け入れシナリオ3と4を検証

### ステップ
1. ダッシュボードを開いたまま待機

2. 新しいノードを登録
   ```bash
   LLMLB_URL=http://localhost:32768 XLLM_PORT=11438 ./node/build/xllm
   ```

3. 期待される結果:
   - **ページをリロードせずに**、5秒以内に新しいノードが一覧に表示される
   - Total Nodes の値が自動的にインクリメントされる
   - Online Nodes の値が自動的にインクリメントされる

### 検証ポイント
- [ ] 5秒以内に自動更新される
- [ ] ページリロード不要
- [ ] ブラウザコンソールでポーリングリクエストが確認できる（`GET /v0/dashboard/nodes`）
- [ ] ポーリング処理が100ms以内に完了（NFR-011）

---

## シナリオ5: ノードオフライン検出

**目的**: オフライン検出の動作確認

### ステップ
1. ダッシュボードを開く

2. 1台のノードを停止
   ```bash
   # server-01のプロセスをCtrl+Cで停止
   ```

3. 60秒以上待機（`LLMLB_NODE_TIMEOUT` のデフォルト値）

4. 期待される結果:
   - ノードのステータスが自動的に「Offline」に変化
   - Offline Nodes の値がインクリメント
   - Online Nodes の値がデクリメント

### 検証ポイント
- [ ] タイムアウト後にステータスが正しく変化
- [ ] UIのステータスバッジがOffline色（グレー）に変化
- [ ] システム統計が正しく更新される

---

## シナリオ6: 手動リフレッシュ

**目的**: リフレッシュボタンの動作確認（Phase 1実装）

### ステップ
1. ダッシュボードページの「Refresh」ボタンをクリック

2. 期待される結果:
   - ノード一覧が即座に更新される
   - システム統計が更新される
   - Loading状態が表示される（短時間）

### 検証ポイント
- [ ] ボタンクリックで即座に更新
- [ ] API呼び出しが確認できる（Network tab）
- [ ] エラーが発生しない

---

## トラブルシューティング

### ダッシュボードが表示されない
**原因**: 静的ファイルが正しく配信されていない
**解決策**:
1. `router/src/web/static/index.html`が存在することを確認
2. `tower-http`のServeDir設定を確認
3. ルーターを再起動

### ノード一覧が空
**原因**: ノードが登録されていない
**解決策**:
1. ノードを起動: `LLMLB_URL=http://localhost:32768 ./node/build/xllm`
2. 登録APIを手動で呼び出し:
   ```bash
   curl -X POST http://localhost:32768/v0/nodes \
     -H "Content-Type: application/json" \
     -d '{
       "machine_name": "test-node",
       "ip_address": "127.0.0.1",
       "runtime_version": "0.1.0",
       "runtime_port": 32768,
       "gpu_available": true,
       "gpu_devices": [{"model":"Test GPU","count":1}]
     }'
   ```

### リアルタイム更新が動作しない
**原因**: JavaScriptのポーリングが失敗
**解決策**:
1. ブラウザのコンソールでエラーを確認
2. Network tabでAPI呼び出しを確認
3. CORSエラーがある場合、Axumの設定を確認

### ステータスが正しく更新されない
**原因**: ヘルスチェックが動作していない
**解決策**:
1. ノードがハートビートを送信しているか確認:
   ```bash
   # ルーターのログを確認（例）
   ls ~/.llmlb/logs/llmlb.jsonl.*
   tail -n 200 ~/.llmlb/logs/llmlb.jsonl.$(date +%Y-%m-%d)
   ```
2. `LLMLB_HEALTH_CHECK_INTERVAL`（または互換の `HEALTH_CHECK_INTERVAL`）環境変数を確認
3. `LLMLB_NODE_TIMEOUT` 環境変数を確認

---

## 次のステップ

クイックスタートが完了したら:

1. **Phase 2: リアルタイム更新の実装**
   - WebSocketまたはポーリングの実装
   - 自動更新ロジックの追加

2. **Phase 3: メトリクス可視化の実装**
   - Chart.js統合
   - CPU/メモリグラフ表示

3. **Phase 4: 高度な機能の実装**
   - フィルタリング＆検索
   - ノード詳細モーダル

---

## テスト実行コマンド

### E2Eテスト実行
```bash
cargo test --test dashboard_e2e
```

### Integration テスト実行
```bash
cargo test --test dashboard_integration
```

### 全テスト実行
```bash
cargo test
```

---

*このクイックスタートガイドは plan.md Phase 1 の成果物です*
