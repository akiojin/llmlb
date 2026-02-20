# 技術リサーチ: IPアドレスロギング＆クライアント分析

**機能ID**: `SPEC-62ac4b68` | **日付**: 2026-02-20

## 既存インフラ調査結果

### サーバー初期化 (ConnectInfo)

- `main.rs:621-627`: `into_make_service_with_connect_info::<SocketAddr>()`が既に設定済み
- axumの`ConnectInfo<SocketAddr>`エクストラクタがハンドラで利用可能
- **追加設定不要**

### データベーススキーマ

- `request_history.client_ip TEXT`: カラム既存（常にNULL）
- `RequestHistoryRow.client_ip: Option<String>`: マッピング既存
- `RequestResponseRecord.client_ip: Option<IpAddr>`: 構造体フィールド既存
- **最新マイグレーション**: `016_add_tps_columns.sql`
- **次のマイグレーション番号**: `017`

### ハンドラ署名

- `openai.rs`の全推論ハンドラ: `State(state)` + `Json(payload)`のみ
- `ConnectInfo<SocketAddr>`パラメータ未追加
- 約15箇所で`client_ip: None`がハードコード

### API認証コンテキスト

- `ApiKeyAuthContext { id: Uuid, created_by: Uuid, permissions, expires_at }`
- リクエストエクステンションに格納済み
- `request.extensions().get::<ApiKeyAuthContext>()`で取得可能

### 設定テーブル

- 現在は存在しない
- システム設定は環境変数で管理
- 閾値のDB永続化には新規テーブルが必要

## 技術的判断

### IP正規化

IPv4-mapped IPv6（`::ffff:x.x.x.x`）はRustの`IpAddr`で簡単に検出・変換可能:

```rust
fn normalize_ip(addr: IpAddr) -> IpAddr {
    match addr {
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                IpAddr::V4(v4)
            } else {
                IpAddr::V6(v6)
            }
        }
        v4 => v4,
    }
}
```

### IPv6 /64グルーピング

SQLiteにはIPv6パース関数がないため、
テキスト操作でプレフィックスを抽出する:

- IPv6アドレス（例: `2001:db8:85a3::8a2e:370:7334`）の先頭4セグメント
- Rust側で正規化した`/64`プレフィックス文字列をDB保存時に生成

### ヒートマップ実装

Recharts 3.xにはヒートマップコンポーネントが存在しない。
CSS Gridベースのカスタム実装が最もシンプル:

- 24列（時間帯）× 7行（曜日）のグリッド
- セルの背景色を値に応じたHSL/透明度で表現
- Tailwind CSSのgridユーティリティで実装
- ホバーでツールチップ表示

### パフォーマンス考慮

集計クエリのパフォーマンスを確保するため:

- `client_ip`にインデックス追加
- 過去24時間の範囲フィルターで`timestamp`インデックス活用
- IPv6グルーピングはアプリケーション層で実施し、
  SQLの複雑さを回避
