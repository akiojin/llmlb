# PoC: openai/gpt-oss-20b (Metal / Apple Silicon)

このPoCは、`openai/gpt-oss-20b` を **llm-router + llm-node (Metal)** で実際にロードして `chat/completions` が返ることを確認します。

## 前提

- macOS (Apple Silicon)
- Xcode + Metal toolchain (`xcrun -sdk macosx -find metal` が成功すること)
- `HF_TOKEN` が環境変数に設定済み（Hugging Face で `openai/gpt-oss-20b` にアクセス可能）

## 実行

```bash
./poc/gpt-oss-metal/run.sh
```

### 注意

- 初回はモデルをダウンロードするため時間とディスク容量が必要です。
- `tmp/poc-gptoss-metal/` 以下にログと作業用ディレクトリを作成します。

