# リサーチ: モデル登録キャッシュとマルチモーダルI/O

## 調査目的

モデル登録のキャッシュ健全性チェックと、画像・音声APIのマルチモーダルルーティングを実現する技術調査。

## キャッシュ健全性チェック

### 問題点

- ダウンロード途中での中断により0Bまたは不完全なファイルが残る
- ファイル存在だけでは有効なキャッシュか判定できない

### 解決策

```rust
fn is_cache_valid(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(meta) => meta.len() > 0,
        Err(_) => false,
    }
}
```

### 判定基準

| 状態 | ファイル存在 | サイズ > 0 | 結果 |
|------|-------------|-----------|------|
| 正常キャッシュ | ✅ | ✅ | 再利用 |
| 不完全キャッシュ | ✅ | ❌ | 再ダウンロード |
| キャッシュなし | ❌ | - | ダウンロード |

## マルチモーダルルーティング

### ランタイムタイプ

```rust
pub enum RuntimeType {
    LlamaCpp,           // LLM推論
    Whisper,            // 音声認識 (ASR)
    StableDiffusion,    // 画像生成
    OnnxRuntime,        // 音声合成 (TTS)
}
```

### APIとランタイムの対応

| API エンドポイント | 必要なランタイム |
|-------------------|-----------------|
| /v1/chat/completions | LlamaCpp |
| /v1/completions | LlamaCpp |
| /v1/audio/transcriptions | Whisper |
| /v1/audio/speech | OnnxRuntime |
| /v1/images/generations | StableDiffusion |
| /v1/images/edits | StableDiffusion |
| /v1/images/variations | StableDiffusion |

### ノード登録時の情報

```json
{
  "runtime_id": "uuid",
  "supported_runtimes": ["llama_cpp", "whisper"],
  "loaded_models": {
    "llm": ["llama-3.1-8b"],
    "asr": ["whisper-large-v3"]
  }
}
```

## ready状態の判定

### 判定フロー

```text
[ModelInfo取得]
     |
     v
[capabilities確認] -- なし --> [404 Not Found]
     |
     v
[対応ノード存在確認] -- なし --> [503 Service Unavailable]
     |
     v
[ノードがready報告] -- false --> [503 Service Unavailable]
     |
     v
[ready=true]
```

### /v1/modelsレスポンスでのready

```json
{
  "id": "llama-3.1-8b",
  "object": "model",
  "ready": true,
  "nodes": [
    {"runtime_id": "...", "ready": true}
  ]
}
```

## 削除時の同期処理

### 削除フロー

```text
[DELETE /v1/models/:name]
     |
     v
[ロードバランサー登録情報削除]
     |
     v
[ノードへ削除通知] --> [ノードがローカルキャッシュ削除]
     |
     v
[200 OK]
```

### 重要: ロードバランサーはバイナリを保持しない

- ロードバランサー: メタデータのみ管理
- ノード: モデルバイナリをローカルキャッシュ

## 参考資料

- SPEC-68551ec8: HuggingFace登録/キャッシュ
- SPEC-617247d2: 音声対応(TTS+ASR)
- SPEC-dcf8677f: capabilities検証
