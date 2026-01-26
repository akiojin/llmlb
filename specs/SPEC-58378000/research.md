# 技術リサーチ: llmlb CLIコマンド

**機能ID**: `SPEC-58378000` | **日付**: 2026-01-08

## 技術調査結果

### 1. 既存CLI構造

**決定**: 既存の`node/src/utils/cli.cpp`を拡張してサブコマンド対応

**理由**:

- 現在のCLIは`-h`, `-V`のみ対応の最小構造
- main.cppが直接サーバーモードを起動する設計
- サブコマンド追加により、serve/run/pull/list等を分岐可能

**検討した代替案**:

- clapライクなC++ライブラリ（CLI11等）→ 依存追加の複雑さを避ける
- 別バイナリ分離 → 単一バイナリの方がユーザビリティ高い

### 2. サーバー・クライアントモデル

**決定**: ollamaスタイルのサーバー常駐＋クライアント接続方式

**理由**:

- `serve`でHTTPサーバーを起動、他コマンドはクライアントとして接続
- 既存の`node/src/api/`に HTTP エンドポイントが存在
- httplib.hを使用した通信基盤が整備済み

**検討した代替案**:

- 毎回プロセス起動 → モデルロード時間のオーバーヘッド大
- Unix socket → プラットフォーム依存性

### 3. REPLの実装

**決定**: readline互換のシンプルなREPL実装

**理由**:

- `/bye`, `/clear`コマンドのみ対応（ollama互換）
- SSE (Server-Sent Events) でストリーミング出力
- 既存の`/v1/chat/completions`エンドポイント活用

**検討した代替案**:

- linenoise → GNU readline → 標準入力で十分

### 4. HuggingFaceモデルダウンロード

**決定**: 既存の`ModelSync`/`ModelDownloader`を拡張

**理由**:

- `node/include/models/model_sync.h`にダウンロード機能が存在
- ETagキャッシュ、プログレス表示機能を流用可能
- HF_TOKEN環境変数でgatedモデル対応

**検討した代替案**:

- huggingface_hub CLI呼び出し → Python依存を避ける
- 新規ダウンローダー実装 → 既存機能の再発明

### 5. ollamaモデル参照

**決定**: `~/.ollama/models/`のmanifest.jsonを解析してblob参照

**理由**:

- 読み取り専用（ollamaのストレージを変更しない）
- manifestにはblobへのsha256参照が含まれる
- `ollama:`プレフィックスで明示的に区別

**検討した代替案**:

- シンボリックリンク → ファイルシステム操作の複雑さ
- コピー → ストレージ二重化の無駄

### 6. プログレス表示

**決定**: ollama互換のプログレスバー形式

**理由**:

- ユーザーの学習コスト最小化
- ANSIエスケープシーケンスで実現可能
- 既存のProgressCallback機構を活用

### 7. 環境変数設計

**決定**: `LLMLB_*`プレフィックスで統一

**理由**:

- `LLMLB_HOST`: サーバー接続先
- `LLMLB_DEBUG`: ログレベル制御
- 既存の`XLLM_*`との整合性

### 8. Vision入力対応

**決定**: 画像パスをプロンプト内で指定（`/path/to/image.png`形式）

**理由**:

- ollama互換のシンプルなUX
- 既存のmultimodal対応エンドポイントを活用
- Base64エンコードで送信

### 9. Reasoning表示制御

**決定**: `--think`/`--hidethinking`フラグで制御

**理由**:

- DeepSeek-R1等のreasoningモデル対応
- `<think>`タグの表示/非表示切り替え
- デフォルトは非表示（`--hidethinking`相当）

## 依存関係確認

### 既存コンポーネント

| コンポーネント | 場所 | 活用方法 |
|---------------|------|----------|
| ModelStorage | `node/include/models/model_storage.h` | モデル一覧・削除 |
| ModelSync | `node/include/models/model_sync.h` | ダウンロード機能 |
| RouterClient | `node/src/api/router_client.cpp` | サーバー通信 |
| HttpServer | `node/src/api/http_server.h` | API提供 |
| LlamaManager | `node/src/core/llama_manager.h` | 推論実行 |

### 新規追加予定

| コンポーネント | 役割 |
|---------------|------|
| CLIParser | サブコマンド解析 |
| CLIClient | サーバー接続クライアント |
| REPLSession | 対話セッション管理 |
| OllamaCompat | ollamaモデル参照 |
| ProgressRenderer | プログレス表示 |

## 技術的リスク

1. **REPLのストリーミング処理**: SSE解析の複雑さ
   - 軽減策: libcurlまたはhttplib.hのストリーミング対応を確認

2. **ollamaフォーマット変更**: manifest構造の変更リスク
   - 軽減策: バージョン検出と互換性レイヤー

3. **Vision入力のファイルサイズ**: 大きな画像のBase64化
   - 軽減策: サイズ制限と圧縮オプション
