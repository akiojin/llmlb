# クイックスタート: IPアドレスロギング＆クライアント分析

**機能ID**: `SPEC-62ac4b68` | **日付**: 2026-02-20

## 前提条件

- Rust toolchain (rustup)
- Node.js + pnpm
- SQLite3
- llmlbが正常にビルド・起動できる環境

## 開発手順

### 1. マイグレーション確認

マイグレーション017が自動適用される。手動操作不要。

```bash
cargo run -- serve
# 起動時にマイグレーションが自動実行される
```

### 2. バックエンドビルド・テスト

```bash
# テスト実行
cargo test

# Clippy
cargo clippy -- -D warnings

# フォーマット
cargo fmt --check
```

### 3. フロントエンドビルド

```bash
# ダッシュボードビルド
pnpm --filter @llm/dashboard build

# 静的ファイルが llmlb/src/web/static/ に出力される
```

### 4. 動作確認

```bash
# サーバー起動（デバッグビルド）
cargo run -- serve

# リクエスト送信（APIキー: sk_debug）
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk_debug" \
  -H "Content-Type: application/json" \
  -d '{"model": "test", "messages": [{"role": "user", "content": "hello"}]}'

# ダッシュボード確認
# ブラウザで http://localhost:8080/dashboard/ を開く
# admin / test でログイン
# Clientsタブを確認
```

### 5. 品質チェック

```bash
make quality-checks
```

## 検証ポイント

1. リクエスト送信後、リクエスト履歴にIPアドレスが記録されている
2. ダッシュボードのHistory タブにClient IPカラムが表示される
3. Clientsタブにランキングテーブルが表示される
4. IPクリックでドリルダウンが展開される
5. 閾値設定がGUIから変更・保存できる
