# Load Test (wrk)

簡易な負荷テストは `scripts/load-test.sh` で実行できます。デフォルトでは `/health` に 30 秒間、128 接続・8 スレッドでリクエストします。

```bash
# wrk が必要 (mac: brew install wrk, linux: apt-get install wrk)

# デフォルト (30s, 128c, 8t)
./scripts/load-test.sh http://127.0.0.1:32769/health

# パラメータ例
DURATION=60s CONNECTIONS=256 THREADS=12 RATE=5000 \
  ./scripts/load-test.sh http://127.0.0.1:32769/health
```

環境変数:
- `DURATION` (例: 60s)
- `CONNECTIONS` (例: 256)
- `THREADS` (例: 12)
- `RATE` wrk の `-R` 相当。0 の場合は無制限。

結果は wrk の標準出力を確認してください。応答メトリクスは `/metrics/prom` で併せて観測できます。
