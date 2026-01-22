# CLI契約: llmlb コマンド

**機能ID**: `SPEC-58378000` | **日付**: 2026-01-08

## コマンド構造

```
llmlb
├── node                      # ノード操作サブコマンド
│   ├── serve                 # サーバー起動
│   ├── run <model>           # REPL起動
│   ├── pull <model>          # ダウンロード
│   ├── list                  # モデル一覧
│   ├── show <model>          # 詳細表示
│   ├── rm <model>            # 削除
│   ├── stop <model>          # アンロード
│   └── ps                    # 実行中一覧
└── router                    # ルーター操作サブコマンド
    ├── nodes                 # ノード一覧
    ├── models                # モデル配信状況
    └── status                # クラスタ状態
```

## node serve

サーバーをフォアグラウンドで起動。

### 使用法

```
llmlb node serve [OPTIONS]
```

### オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| --port | リッスンポート | 32769 |
| --host | バインドアドレス | 0.0.0.0 |

### 環境変数

| 変数 | 説明 |
|------|------|
| XLLM_PORT | ポート番号 |
| XLLM_BIND_ADDRESS | バインドアドレス |

### 終了コード

| コード | 条件 |
|--------|------|
| 0 | 正常終了 (SIGINT/SIGTERM) |
| 1 | 起動失敗 |

---

## node run

モデルとの対話REPLを起動。

### 使用法

```
llmlb node run <MODEL> [OPTIONS]
```

### 引数

| 引数 | 説明 | 必須 |
|------|------|------|
| MODEL | モデル名またはエイリアス | Yes |

### オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| --think | 思考過程を表示 | false |
| --hidethinking | 思考過程を非表示 | true |

### REPLコマンド

| コマンド | 説明 |
|---------|------|
| /bye | REPLを終了 |
| /clear | 会話履歴をクリア |

### Vision入力

```
>>> /path/to/image.png 質問テキスト
```

### 終了コード

| コード | 条件 |
|--------|------|
| 0 | 正常終了 (/bye) |
| 1 | エラー (モデルが見つからない等) |
| 2 | サーバー接続エラー |

---

## node pull

HuggingFaceからモデルをダウンロード。

### 使用法

```
llmlb node pull <MODEL>
```

### 引数

| 引数 | 説明 | 形式 |
|------|------|------|
| MODEL | モデル指定 | `owner/model` または HuggingFace URL |

### 環境変数

| 変数 | 説明 |
|------|------|
| HF_TOKEN | gatedモデル用認証トークン |

### プログレス出力

```
pulling manifest ✓
pulling abc123def456... 45% ▓▓▓▓▓▓▓▓░░░░░░░░░░░░ 2.9 GB/6.4 GB
```

### 終了コード

| コード | 条件 |
|--------|------|
| 0 | ダウンロード成功 |
| 1 | ダウンロード失敗 (ネットワークエラー、認証エラー等) |
| 2 | サーバー接続エラー |

---

## node list

ローカルに保存されているモデルの一覧を表示。

### 使用法

```
llmlb node list
```

### 出力形式

```
NAME                                    SIZE     MODIFIED
meta-llama/Llama-3.2-3B-Instruct       6.4 GB   2 hours ago
deepseek/deepseek-r1-0528              14.2 GB  1 day ago
ollama:llama3.2 (readonly)             4.1 GB   3 days ago
```

### 終了コード

| コード | 条件 |
|--------|------|
| 0 | 成功 |
| 2 | サーバー接続エラー |

---

## node show

モデルの詳細情報を表示。

### 使用法

```
llmlb node show <MODEL> [OPTIONS]
```

### オプション

| オプション | 説明 |
|-----------|------|
| --license | ライセンス情報のみ表示 |
| --parameters | パラメータ情報のみ表示 |
| --modelfile | モデルファイル情報 |
| --template | チャットテンプレート |
| --system | システムプロンプト |

### 出力形式

```
Model: meta-llama/Llama-3.2-3B-Instruct
  Architecture: llama
  Parameters: 3.21B
  Quantization: Q4_K_M
  Context Length: 131072
  Format: GGUF
  Size: 6.4 GB
  License: Llama 3.2 Community License
```

### 終了コード

| コード | 条件 |
|--------|------|
| 0 | 成功 |
| 1 | モデルが見つからない |
| 2 | サーバー接続エラー |

---

## node rm

モデルを削除（確認なし）。

### 使用法

```
llmlb node rm <MODEL>
```

### 終了コード

| コード | 条件 |
|--------|------|
| 0 | 削除成功 |
| 1 | モデルが見つからない、または削除失敗 |
| 2 | サーバー接続エラー |

---

## node stop

実行中のモデルをアンロード。

### 使用法

```
llmlb node stop <MODEL>
```

### 終了コード

| コード | 条件 |
|--------|------|
| 0 | アンロード成功 |
| 1 | モデルがロードされていない |
| 2 | サーバー接続エラー |

---

## node ps

実行中のモデル一覧を表示。

### 使用法

```
llmlb node ps
```

### 出力形式

```
NAME                              ID       SIZE     PROCESSOR  UNTIL        VRAM    TEMP
meta-llama/Llama-3.2-3B-Instruct  abc123   6.4 GB   100% GPU   4 minutes    85%     62°C
```

### カラム説明

| カラム | 説明 |
|--------|------|
| NAME | モデル名 |
| ID | 短縮ID |
| SIZE | メモリ使用量 |
| PROCESSOR | GPU/CPU使用率 |
| UNTIL | アイドルタイムアウトまでの時間 |
| VRAM | VRAM使用率 |
| TEMP | GPU温度 |

### 終了コード

| コード | 条件 |
|--------|------|
| 0 | 成功 |
| 2 | サーバー接続エラー |

---

## router nodes

登録されているノードの一覧を表示。

### 使用法

```
llmlb router nodes
```

### 出力形式

```
ID         HOST              STATUS    GPU      MODELS
abc123     192.168.1.10     online    RTX4090  3
def456     192.168.1.11     online    A100     5
```

### 終了コード

| コード | 条件 |
|--------|------|
| 0 | 成功 |
| 2 | ルーター接続エラー |

---

## router models

各ノードで利用可能なモデル一覧を表示。

### 使用法

```
llmlb router models
```

### 出力形式

```
MODEL                              NODES    STATUS
meta-llama/Llama-3.2-3B-Instruct  2        available
deepseek/deepseek-r1-0528         1        available
gpt-oss/gpt-oss-20b               0        unavailable
```

### 終了コード

| コード | 条件 |
|--------|------|
| 0 | 成功 |
| 2 | ルーター接続エラー |

---

## router status

クラスタ全体の状態サマリーを表示。

### 使用法

```
llmlb router status
```

### 出力形式

```
Cluster Status: healthy

Nodes:      3 online / 0 offline
Models:     12 available
Requests:   1,234 (last hour)
VRAM:       85% average utilization
```

### 終了コード

| コード | 条件 |
|--------|------|
| 0 | 成功 |
| 2 | ルーター接続エラー |
