# リサーチ: gpt-oss-20b safetensors 実行

## 調査目的

gpt-oss-20b を safetensors 形式で GPU 実行するための技術調査。

## 技術スタック

### 推論エンジン候補

| エンジン | Metal | DirectML | 備考 |
|---------|-------|----------|------|
| llama.cpp | ○ | ○ | GGUF必須、safetensors非対応 |
| MLX | ○ | × | Apple専用、safetensors対応 |
| ONNX Runtime | △ | ○ | 変換必要 |
| カスタム実装 | ○ | ○ | 開発コスト大 |

### 選定方針

- Metal: MLX または カスタムsafetensorsローダー
- DirectML: 公式GPU最適化アーティファクト必須（初期実装）

## safetensors シャーディング

### 構造

```text
model.safetensors.index.json
├── weight_map: { "layer.0.weight": "model-00001-of-00003.safetensors", ... }
└── metadata: { "total_size": 40000000000 }

model-00001-of-00003.safetensors
model-00002-of-00003.safetensors
model-00003-of-00003.safetensors
```

### 検証項目

- index.json の weight_map が全 shard を参照
- shard ファイルが全て存在
- 各 shard のハッシュ検証（オプション）

## 公式GPU最適化アーティファクト

### 許可リスト

- `openai/*`
- `nvidia/*`

### 取得フロー

1. 登録時に許可リスト内のリポジトリを検索
2. GPU バックエンド（Metal/DirectML）に対応するファイルを特定
3. マニフェストに実行キャッシュとして追加

## 必須メタデータ

| ファイル | 必須 | 用途 |
|---------|------|------|
| config.json | ○ | モデル構成、hidden_size, num_layers等 |
| tokenizer.json | ○ | トークナイズ |
| chat_template.jinja | △ | チャットフォーマット（なければデフォルト使用） |

## 参考資料

- [Hugging Face safetensors](https://huggingface.co/docs/safetensors)
- [MLX Documentation](https://ml-explore.github.io/mlx/)
- [DirectML Documentation](https://docs.microsoft.com/en-us/windows/ai/directml/)
