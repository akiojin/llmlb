# 技術リサーチ: モデル自動解決機能

## リサーチ課題

1. モデル解決戦略の設計
2. 外部ソース（HuggingFace）からのダウンロード実装
3. 重複ダウンロード防止メカニズム
4. 進捗通知方式

## 1. モデル解決フロー設計

### 決定

ローカルファースト + 外部ソースフォールバック方式を採用。

### 理由

- ローカルキャッシュを優先することで、ネットワーク遅延を回避
- 外部ソースへの直接アクセスにより、ルーターの負荷を軽減
- `supported_models.json`による明示的なモデル定義で、予期しないダウンロードを防止

### 解決優先順位

```text
1. ローカルストレージ (~/.llmlb/models/)
2. 外部ソース（HuggingFace等）から直接取得
3. ルータープロキシ経由取得（外部アクセス不可時のフォールバック）
4. エラー（supported_models.json未定義）
```

### 代替案比較表

| 方式 | メリット | デメリット | 採用 |
|------|----------|------------|------|
| ローカルファースト | 高速、オフライン対応 | 初回ダウンロード必要 | ✅ |
| ルーター経由のみ | 一元管理 | ルーター負荷大、SPOF | ❌ |
| オンデマンドのみ | ストレージ節約 | 毎回ダウンロード | ❌ |

## 2. HuggingFaceダウンロード実装

### 決定

C++側でhttplibを使用したHTTPダウンロード実装。

### 理由

- llama.cpp自体がHTTPSダウンロード機能を持たないため
- httplibは軽量で、既存のNode依存関係と互換性がある
- ストリーミングダウンロードで大容量ファイルに対応

### 実装方法

```cpp
// ダウンロードフロー概要
1. supported_models.jsonからモデル定義を取得
2. HuggingFace Hub APIでアーティファクトURL取得
3. httplibでストリーミングダウンロード
4. 進捗コールバックで通知
5. ローカルストレージに保存
```

### HuggingFace Hub API

- エンドポイント: `https://huggingface.co/{repo_id}/resolve/{revision}/{filename}`
- 認証: `HF_TOKEN`環境変数（オプション、プライベートリポジトリ用）
- レスポンス: バイナリストリーム

## 3. 重複ダウンロード防止

### 決定

ロックファイル + インメモリダウンロード状態管理。

### 理由

- ファイルベースのロックで、プロセス再起動後も状態を維持
- インメモリ状態で、同一プロセス内の重複を即時検出
- 待機リクエストはダウンロード完了を待機

### 実装方法

```cpp
// 重複防止フロー
1. モデルパスに対するロックファイル (.lock) を確認
2. ロックが存在する場合
   - 別プロセスがダウンロード中
   - ロック解除を待機（タイムアウト: 5分）
3. ロックが存在しない場合
   - ロックファイルを作成
   - ダウンロード実行
   - 完了後ロック解除
```

### ロックファイル形式

```text
~/.llmlb/models/.locks/{model_id}.lock
内容: PID, 開始時刻, 進捗率
```

## 4. 進捗通知方式

### 決定

コールバック関数 + ログ出力方式。

### 理由

- 非同期処理との親和性が高い
- ログ出力で運用監視にも対応
- 将来的なWebSocket通知への拡張が容易

### 実装方法

```cpp
using ProgressCallback = std::function<void(float progress, size_t downloaded, size_t total)>;

void download_model(const std::string& model_id, ProgressCallback callback) {
    // 10%単位で進捗通知
    size_t last_reported = 0;
    auto on_progress = [&](size_t downloaded, size_t total) {
        size_t current = (downloaded * 100) / total;
        if (current >= last_reported + 10) {
            callback(current / 100.0f, downloaded, total);
            last_reported = current;
        }
    };
    // ダウンロード実行...
}
```

## 参考リソース

- [HuggingFace Hub API](https://huggingface.co/docs/hub/api)
- [httplib C++ library](https://github.com/yhirose/cpp-httplib)
- [llama.cpp model loading](https://github.com/ggerganov/llama.cpp)
