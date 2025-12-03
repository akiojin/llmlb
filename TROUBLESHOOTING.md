# TROUBLESHOOTING

## 起動時に GPU が見つからない
- 確認: `nvidia-smi` または `CUDA_VISIBLE_DEVICES`
- 環境変数で無効化: ノード側 `LLM_ALLOW_NO_GPU=true`（デフォルトは禁止）
- それでも失敗する場合は NVML ライブラリの有無を確認

## クラウドモデルが 401/400 を返す
- ルーター側で `OPENAI_API_KEY` / `GOOGLE_API_KEY` / `ANTHROPIC_API_KEY` が設定されているか確認
- ダッシュボード `/api/dashboard/stats` の `*_key_present` が false なら未設定
- プレフィックスなしモデルはローカルにルーティングされるので、クラウドキーなしで利用したい場合はプレフィックスを付けない

## ポート競合で起動しない
- ルーター: `ROUTER_PORT` を変更（例: `ROUTER_PORT=18080`）
- ノード: `LLM_NODE_PORT` または `--port` で変更

## SQLite ファイル作成に失敗
- `DATABASE_URL` のパス先ディレクトリの書き込み権限を確認
- Windows の場合はパスにスペースが含まれていないか確認

## ダッシュボードが表示されない
- ブラウザキャッシュをクリア
- バンドル済み静的ファイルが壊れていないか `cargo clean` → `cargo run` を試す
- リバースプロキシ経由の場合は `/dashboard/*` の静的配信設定を確認

## OpenAI互換APIで 503 / モデル未登録
- 全ノードが `initializing` の場合 503 を返すことがあります。ノードのモデルロードを待つか、`/api/dashboard/agents` で状態を確認
- モデル指定がローカルに存在しない場合、ノードが自動プルするまで待機

## ログが多すぎる / 少なすぎる
- 環境変数 `RUST_LOG` で制御（例: `RUST_LOG=info` または `RUST_LOG=or_router=debug`）
- ノードのログは `spdlog` で出力。構造化ログは `tracing_subscriber` でJSON設定可
