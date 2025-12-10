# クイックスタート: HF GGUFモデル登録・ダウンロード

## 前提
- Router が起動済み
- HF へのネットワーク到達性あり（必要なら `HF_TOKEN` を設定、社内ミラー利用時は `HF_BASE_URL` を上書き）
- ノードは manifest に従い自己ダウンロード可能

## 1. カタログを確認
- Web: 「モデル管理」→「対応可能モデル（HF）」タブを開く。検索でキーワード入力。
- CLI: `llm-router model list --search llama --limit 10`

## 2. 対応モデルに登録
- Web: 対応可能リストから対象GGUFを選び「登録」。
- CLI: `llm-router model add TheBloke/Llama-2-7B-GGUF --file llama-2-7b.Q4_K_M.gguf`
- 成功すると /v1/models にIDが追加される。
- レスポンス `warnings` にGPUメモリ不足の可能性が出た場合は、より小さいバリアントを選択する。

## 3. ダウンロードを指示
- Web: 対応モデルタブでモデルを選択し「今すぐダウンロード」→「全ノード」または「指定ノード」を選ぶ。
- CLI 全ノード: `llm-router model download hf/TheBloke/Llama-2-7B-GGUF/llama-2-7b.Q4_K_M.gguf --all`
- CLI 指定ノード: `llm-router model download <name> --node <uuid>`
- タスクIDが返る。

## 4. 進捗確認
- Web: ダウンロードタスクリストに進捗が表示される（5秒間隔）。
- CLI: `llm-router task show <task_id>` または再度 download コマンドで進捗取得。

## 5. 推論で利用
- OpenAI互換エンドポイントで `model` に登録IDを指定して実行。モデルが未ロードならオンデマンドロードされる。

## トラブルシュート
- HF 429/ダウン: CLI `--format json` で `cached:true` が返る。トークン設定または時間をおく。
- ダウンロード失敗: タスクの `error` を確認。容量不足/URL不可の場合は別モデルを選定。
