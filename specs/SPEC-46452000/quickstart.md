# クイックスタート: LM Studioエンドポイントタイプ検出

**機能ID**: `SPEC-46452000` | **日付**: 2026-02-13

## 前提条件

- Rust toolchain (cargo, rustfmt, clippy)
- LM Studio 0.4.0以降（実機検証用）
- pnpm (markdownlint用)

## 開発手順

### 1. 変更の概要を把握

```text
llmlb/src/types/endpoint.rs     # EndpointType enum拡張
llmlb/src/detection/mod.rs      # 検出順序変更
llmlb/src/detection/lm_studio.rs # LM Studio検出（新規）
llmlb/src/metadata/mod.rs       # ModelMetadata拡張 + ルーティング
llmlb/src/metadata/lm_studio.rs # メタデータ取得（新規）
llmlb/src/sync/mod.rs           # 同期対象追加
llmlb/src/api/endpoints.rs      # API応答にlm_studio対応
```

### 2. TDDサイクル開始

```bash
# RED: テストを書く → 失敗確認
cargo test 2>&1 | grep -E "FAILED|test result"

# GREEN: 最小実装
# ... 実装 ...

# REFACTOR: 品質向上
cargo fmt --check
cargo clippy -- -D warnings
```

### 3. 品質チェック

```bash
make quality-checks 2>&1 | tail -50
```

### 4. 実機検証（LM Studio）

```bash
# LM Studioサーバー起動
lms server start

# 検出確認
curl http://localhost:1234/api/v1/models | jq .

# エンドポイント登録テスト
curl -X POST http://localhost:3000/api/endpoints \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_debug" \
  -d '{"name": "LM Studio Local", "base_url": "http://localhost:1234"}'
```
