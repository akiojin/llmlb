# Quickstart: GPU必須ノード登録チェックリスト

1. **ノードを起動する**

   GPU搭載マシンでノードを起動する。ログに GPU 検出が出力され、登録リクエストに `gpu_devices` が含まれていることを確認する。

2. **登録APIを検証する**

   `POST /api/nodes` へ次のJSONを送信する。

   ```json
   {
     "machine_name": "gpu-node-1",
     "ip_address": "10.0.0.10",
     "runtime_version": "0.1.30",
     "runtime_port": 32768,
     "gpu_available": true,
     "gpu_devices": [
       { "model": "NVIDIA RTX 4090", "count": 2 }
     ]
   }
   ```

   成功時は `status: "registered"` が返り、`GET /api/nodes` のレスポンスに `gpu_devices` が含まれる。`gpu_devices: []` などGPU情報が欠損したリクエストは 403 と `{"error":"Validation error: GPU hardware is required"}` を返す。

3. **ストレージクリーンアップを確認する**

   過去バージョンで登録された GPU 非搭載ノード（`gpu_available=false` または GPU 情報欠損）が DB に残っている状態でロードバランサーを起動する。起動ログに `Removing GPU-less node from database during startup cleanup` が表示され、`GET /api/nodes` から当該ノードが消えていることを確認する。

4. **ダッシュボードの表示を確認する**

   `/dashboard/` を開き、テーブルとモーダルで `GPU NVIDIA RTX 4090 (2枚)` のようにモデル名と枚数が表示されることを確認する。また `GET /api/dashboard/nodes` のレスポンスにも `gpu_devices` 配列が含まれることを確認する。

5. **ローカル検証を実行する**

   `make quality-checks`
