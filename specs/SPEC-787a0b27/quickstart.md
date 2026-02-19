# クイックスタート: llmlb serveコマンドのシングル実行制約

**機能ID**: `SPEC-787a0b27` | **日付**: 2026-01-30

## 概要

llmlbのシングル実行制約機能は、同一ポートでのサーバー重複起動を防止します。
この機能により、ポート競合やデータ破損を回避できます。

## 基本的な使用方法

### サーバーの起動

```bash
# デフォルトポート（32768）で起動
llmlb serve

# 特定のポートで起動
llmlb serve --port 8000

# ホストとポートを指定
llmlb serve --host 127.0.0.1 --port 8000
```

### サーバーの状態確認

```bash
# 全ての起動中サーバーを表示
llmlb status

# 特定ポートの状態を確認
llmlb status --port 8000

# JSON形式で出力
llmlb status --format json
```

出力例:

```text
PORT    PID     STARTED                 STATUS
8000    12345   2026-01-30 12:00:00    Running
9000    12346   2026-01-30 11:30:00    Running
```

### サーバーの停止

```bash
# 特定ポートのサーバーを停止
llmlb stop --port 8000

# 強制停止（確認なし）
llmlb stop --port 8000 --force
```

## 動作シナリオ

### シナリオ1: 重複起動の防止

```bash
# ターミナル1: サーバー起動
$ llmlb serve --port 8000
[INFO] LLM Load Balancer server listening on 0.0.0.0:8000

# ターミナル2: 同じポートで起動を試行
$ llmlb serve --port 8000
Error: Server already running on port 8000 (PID: 12345, started: 2026-01-30T12:00:00Z)

To stop: llmlb stop --port 8000
Or:      kill -TERM 12345
```

### シナリオ2: 異なるポートでの複数起動

```bash
# ターミナル1
$ llmlb serve --port 8000
[INFO] LLM Load Balancer server listening on 0.0.0.0:8000

# ターミナル2
$ llmlb serve --port 9000
[INFO] LLM Load Balancer server listening on 0.0.0.0:9000

# 状態確認
$ llmlb status
PORT    PID     STARTED                 STATUS
8000    12345   2026-01-30 12:00:00    Running
9000    12346   2026-01-30 12:01:00    Running
```

### シナリオ3: クラッシュ後の復旧

```bash
# サーバーがクラッシュ（kill -9でシミュレート）
$ kill -9 12345

# ロックファイルは残っているが、新しいサーバーは起動可能
$ llmlb serve --port 8000
[WARN] Stale lock file detected (PID 12345 not running), cleaning up
[INFO] LLM Load Balancer server listening on 0.0.0.0:8000
```

### シナリオ4: グレースフルシャットダウン

```bash
# Ctrl+C または SIGTERM で安全に停止
$ llmlb serve --port 8000
[INFO] LLM Load Balancer server listening on 0.0.0.0:8000
^C
[INFO] Received shutdown signal, cleaning up...
[INFO] Server stopped gracefully
```

## 環境変数

| 変数名 | 説明 | デフォルト |
|--------|------|-----------|
| `LLMLB_PORT` | リッスンポート | 32768 |
| `LLMLB_HOST` | バインドアドレス | 0.0.0.0 |

```bash
# 環境変数を使用した起動
LLMLB_PORT=8000 llmlb serve
```

## ロックファイルの場所

| OS | パス |
|----|------|
| Linux/macOS | `/tmp/llmlb/serve_{port}.lock` |
| Windows | `%TEMP%\llmlb\serve_{port}.lock` |

## トラブルシューティング

### ロックファイルが残っている場合

通常はPID検証で自動的にクリーンアップされますが、
手動で削除する場合:

```bash
# Linux/macOS
rm /tmp/llmlb/serve_8000.lock

# Windows
del %TEMP%\llmlb\serve_8000.lock
```

### 起動中のサーバーが見つからない場合

```bash
$ llmlb stop --port 8000
Error: No server running on port 8000

# プロセスを直接確認
$ ps aux | grep llmlb
$ lsof -i :8000  # macOS/Linux
```

## 検証手順

1. **単一起動テスト**: `llmlb serve --port 8000` でサーバー起動を確認
2. **重複起動テスト**: 別ターミナルで同じコマンドを実行し、エラーを確認
3. **状態確認テスト**: `llmlb status` で起動中サーバーを確認
4. **停止テスト**: `llmlb stop --port 8000` でサーバー停止を確認
5. **クラッシュ復旧テスト**: `kill -9` 後に再起動可能なことを確認
