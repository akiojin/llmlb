# リサーチ: CLI インターフェース整備

## 調査目的

Router/Node の CLI インターフェースを整備するための技術調査。

## CLIライブラリ選定

### 候補比較

| ライブラリ | 言語 | メリット | デメリット |
|-----------|------|---------|-----------|
| clap | Rust | 標準的、derive対応 | 依存追加 |
| argh | Rust | シンプル | 機能限定的 |
| structopt | Rust | 直感的 | clap統合済み |
| 標準ライブラリ | C++ | 依存なし | 手動実装 |

### 選定結果

- **Router (Rust)**: clap v4 (derive マクロ)
- **Node (C++)**: 標準ライブラリ（argc/argv手動パース）

### clap採用理由

- Rust CLIの事実上の標準
- derive マクロで宣言的に定義可能
- 自動ヘルプ生成
- 豊富なドキュメント

## 環境変数プレフィックス統一

### 現状の問題

- 環境変数名が不統一（`LLMLB_PORT`, `JWT_SECRET`, `LLM_LOG_LEVEL` など）
- コンポーネント判別が困難

### 解決策

| コンポーネント | プレフィックス |
|---------------|---------------|
| Router | `LLMLB_*` |
| Node | `XLLM_*` |

### マイグレーション戦略

```rust
fn get_env_with_fallback(new_name: &str, old_name: &str) -> Option<String> {
    if let Ok(val) = std::env::var(new_name) {
        return Some(val);
    }
    if let Ok(val) = std::env::var(old_name) {
        tracing::warn!("{} is deprecated, use {} instead", old_name, new_name);
        return Some(val);
    }
    None
}
```

## JWT_SECRET永続化

### 現状の問題

- 環境変数で設定（設定されない場合はハードコード）
- 再起動時に異なるシークレットだと既存トークン無効化

### 解決策

1. 初回起動時にUUIDv4で自動生成
2. `~/.llmlb/jwt_secret` に保存（パーミッション600）
3. 以降の起動時はファイルから読み込み
4. 環境変数で上書き可能

### 優先順位

1. 環境変数 `LLMLB_JWT_SECRET`
2. ファイル `~/.llmlb/jwt_secret`
3. 自動生成（ファイルに保存）

## バージョン情報の取得

### Router (Rust)

```rust
const VERSION: &str = env!("CARGO_PKG_VERSION");
```

### Node (C++)

```cpp
// node/include/utils/version.h
#define XLLM_VERSION "0.1.0"
```

CMake連携でビルド時に自動設定も可能。

## CLIオプション設計原則

### 設計哲学

> **CLIはサーバー起動専用とし、すべての管理操作はAPI/Dashboard経由で行う。**

### 対応オプション

- `-h, --help`: ヘルプ表示
- `-V, --version`: バージョン表示

### 廃止された機能

- `user` サブコマンド → API `/v0/users/*`
- `model` サブコマンド → API `/v1/models/*`
- `--preload-model` → Dashboard経由

## 参考資料

- [clap Documentation](https://docs.rs/clap/latest/clap/)
- [Conventional CLI Guidelines](https://clig.dev/)
- SPEC-1970e39f: 構造化ロギング強化
