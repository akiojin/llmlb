# 技術リサーチ: 画像認識モデル対応（Image Understanding）

## リサーチ課題

1. マルチモーダルモデルのアーキテクチャと対応エンジン
2. 画像前処理パイプラインの設計
3. OpenAI Vision API互換性の実現方法

## 1. マルチモーダルモデルとエンジン

### 決定

**llama.cpp multimodal** と **GGUF形式のVisionモデル** を採用

### 理由

- llama.cppがLLaVA、Qwen-VL等の主要Visionモデルをサポート
- 既存のllama.cppベースノードとの統合が容易
- GGUF形式で統一されたモデル管理が可能

### 代替案比較表

| エンジン | 対応モデル | 統合難易度 | パフォーマンス | 判定 |
|---------|-----------|-----------|--------------|------|
| llama.cpp (clip.cpp) | LLaVA, Qwen-VL, Phi-3-vision | 低 | 高 | ○ |
| vLLM | LLaVA, Qwen-VL | 中 | 最高 | △ |
| Hugging Face Transformers | 全て | 高 | 中 | × |
| TensorRT-LLM | 一部 | 高 | 最高 | × |

### 対応Visionモデル一覧

| モデル | サイズ | 画像解像度 | 特徴 |
|-------|--------|-----------|------|
| LLaVA-1.6 | 7B/13B/34B | 336x336〜672x672 | 汎用性が高い |
| Qwen-VL-Chat | 7B | 448x448 | 中国語対応 |
| Phi-3-Vision | 4.2B | 1344x1344 | 軽量高精度 |
| MiniCPM-V | 3B | 1344x1344 | 最軽量 |

## 2. 画像前処理パイプライン

### 決定

**ルーター側でBase64デコード → ノード側でリサイズ・正規化** を採用

### 理由

- ルーターでデコードすることで、バリデーションとエラー検出を早期に実行
- ノード側でモデル固有のリサイズ処理を実行（モデルによる最適解像度が異なる）
- ネットワーク転送量は増加するが、処理の分離が明確

### 代替案比較表

| 方式 | 長所 | 短所 | 判定 |
|------|------|------|------|
| ルーターで完全処理 | エラー検出早い | モデル依存処理が困難 | × |
| ノードで完全処理 | モデル最適化可能 | エラー検出遅い | × |
| ハイブリッド（採用） | バランス良い | 処理分散 | ○ |
| 画像プロキシサービス | 柔軟 | 追加コンポーネント | × |

### 画像処理フロー

```text
Client
   │
   ▼ (1) 画像URL or Base64
Router
   │ (2) Base64デコード、バリデーション
   │     - フォーマット検証 (JPEG/PNG/GIF/WebP)
   │     - サイズ検証 (max 10MB)
   │     - 破損検出
   │
   ▼ (3) 生画像データ
Node
   │ (4) モデル固有処理
   │     - リサイズ (モデル解像度に合わせる)
   │     - 正規化 (pixel値を-1〜1に)
   │     - テンソル化
   │
   ▼ (5) 推論実行
Model
```

## 3. OpenAI Vision API互換性

### 決定

**OpenAI Vision APIのサブセット実装** を採用

### 理由

- 既存のOpenAIクライアントライブラリがそのまま使用可能
- 学習コストが低い
- 将来的な拡張が容易

### OpenAI Vision API形式

```json
{
  "model": "gpt-4-vision-preview",
  "messages": [
    {
      "role": "user",
      "content": [
        {"type": "text", "text": "この画像は何ですか？"},
        {
          "type": "image_url",
          "image_url": {
            "url": "https://example.com/image.jpg",
            "detail": "auto"
          }
        }
      ]
    }
  ],
  "max_tokens": 300
}
```

### 実装範囲

| 機能 | OpenAI | 本実装 | 備考 |
|------|--------|--------|------|
| 画像URL | ○ | ○ | 必須 |
| Base64画像 | ○ | ○ | 必須 |
| 複数画像 | ○ | ○ | 最大10枚 |
| detail パラメータ | ○ | △ | 無視（auto扱い） |
| ストリーミング | ○ | ○ | SSE |

## 4. 画像URL取得の考慮事項

### 決定

**reqwestによる非同期取得、タイムアウト30秒、リダイレクト3回まで** を採用

### セキュリティ考慮

| リスク | 対策 |
|--------|------|
| SSRF攻撃 | プライベートIP/ローカルホスト拒否 |
| 巨大ファイル | Content-Lengthチェック、ストリーミング |
| 不正リダイレクト | リダイレクト回数制限 |
| DNS Rebinding | 解決済みIPのホワイトリスト検証 |

### 実装方法

```rust
async fn fetch_image(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()?;

    let response = client.get(url).send().await?;

    // Content-Lengthチェック
    if let Some(len) = response.content_length() {
        if len > MAX_IMAGE_SIZE {
            return Err(ImageError::TooLarge);
        }
    }

    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
}
```

## 参考リソース

- [OpenAI Vision API Documentation](https://platform.openai.com/docs/guides/vision)
- [llama.cpp Multimodal Support](https://github.com/ggerganov/llama.cpp/tree/master/examples/llava)
- [LLaVA Model Zoo](https://github.com/haotian-liu/LLaVA)
- [image-rs (Rust画像処理)](https://docs.rs/image/latest/image/)
