# Benchmarks

Router + local/クラウド経路の性能を測るための手順メモ。実行結果は
`benchmarks/results/YYYYMMDD-<run>.md` に残してください。

## 1. 前提
- Router 起動済み (`ROUTER_PORT` デフォルト 8080)
- ローカル LLM ノードが最低 1 台オンライン
- クラウドキーを試す場合: `OPENAI_API_KEY` / `GOOGLE_API_KEY` /
  `ANTHROPIC_API_KEY`
- 負荷ツール: `wrk` または `hey` がインストール済み

## 2. シナリオ
1. **ローカル経路 (ベースライン)**  
   - モデル: `gpt-oss:20b` などプレフィックスなし
   - 目的: ローカル LLM のスループット/レイテンシ基準を取得
2. **クラウド経路 (prefix)**  
   - モデル: `openai:gpt-4o` / `google:gemini-1.5-pro` / `anthropic:claude-3-opus`
   - 目的: クラウド転送のオーバーヘッド可視化
3. **同時接続スケール**  
   - 5/20/50/100 接続でスループットと p95/p99 を比較
4. **長時間安定性 (30〜60分)**  
   - GC / メモリリーク / 接続切断有無を確認

## 3. コマンド例
```bash
# wrk でローカル経路 (10スレッド, 50接続, 30秒)
WRK_TARGET=http://localhost:8080 \
scripts/benchmarks/run_wrk.sh \
  -t10 -c50 -d30s \
  --script scripts/benchmarks/chat.lua

# hey でクラウド経路 (openai:)
hey -n 200 -c 20 -m POST \
  -H "Content-Type: application/json" \
  -d '{"model":"openai:gpt-4o","messages":[{"role":"user","content":"ping"}]}' \
  http://localhost:8080/v1/chat/completions
```

`chat.lua` は wrk のチャット用 JSON ボディサンプル（scripts/benchmarks/chat.lua）。

## 4. 計測指標
- スループット: `Requests/sec`
- レイテンシ: 平均 / p50 / p90 / p95 / p99
- 失敗率: エラー応答数 / タイムアウト数
- リソース: Router/ノードの CPU/GPU/メモリ（別途 `htop`, `nvidia-smi`）

## 5. 記録テンプレート
`benchmarks/results/YYYYMMDD-<run>.md` で以下を残す:

```
## ラン名
- 日時: 2025-11-25 12:34 JST
- 対象: ローカル / openai / google / anthropic
- コマンド: wrk -t10 -c50 -d30s ...
- 成果物:
  - RPS: xxx
  - p95: xxx ms / p99: xxx ms
- 失敗: xxx (内訳 4xx/5xx/timeout)
- 備考: 例) GPU使用率70%、クラウド側403なし
```

### wrk出力のCSV化
```bash
wrk ... --latency | scripts/benchmarks/wrk_parse.py --label local \
  > benchmarks/results/20251125-local.csv
```
CSV列: `label,rps,p50_ms,p75_ms,p90_ms,p95_ms,p99_ms,non2xx,socket_errors,requests,duration_s`

## 6. 次ステップ
- 主要シナリオで baseline を取って results に保存
- p95/p99 をグラフ化する場合は `benchmarks/results/*.csv` を生成し、
  Grafana で可視化する
