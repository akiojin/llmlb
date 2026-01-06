# データモデル: 画像生成モデル対応

## エンティティ定義

### ImageGenerationRequest

テキストから画像生成のリクエスト。

```rust
pub struct ImageGenerationRequest {
    /// 画像生成プロンプト
    pub prompt: String,
    /// 使用モデル
    pub model: Option<String>,
    /// 生成枚数（1-10）
    pub n: Option<u8>,
    /// 画像品質
    pub quality: Option<ImageQuality>,
    /// 出力形式
    pub response_format: Option<ImageResponseFormat>,
    /// 画像サイズ
    pub size: Option<ImageSize>,
    /// スタイル
    pub style: Option<ImageStyle>,
    /// ユーザー識別子
    pub user: Option<String>,
}
```

### ImageEditRequest

画像編集（Inpainting）のリクエスト。

```rust
pub struct ImageEditRequest {
    /// 編集対象の画像（PNG、最大4MB）
    pub image: Vec<u8>,
    /// 編集プロンプト
    pub prompt: String,
    /// マスク画像（オプション）
    pub mask: Option<Vec<u8>>,
    /// 使用モデル
    pub model: Option<String>,
    /// 生成枚数
    pub n: Option<u8>,
    /// 画像サイズ
    pub size: Option<ImageSize>,
    /// 出力形式
    pub response_format: Option<ImageResponseFormat>,
}
```

### ImageVariationRequest

画像バリエーション生成のリクエスト。

```rust
pub struct ImageVariationRequest {
    /// 元画像（PNG、最大4MB）
    pub image: Vec<u8>,
    /// 使用モデル
    pub model: Option<String>,
    /// 生成枚数
    pub n: Option<u8>,
    /// 出力形式
    pub response_format: Option<ImageResponseFormat>,
    /// 画像サイズ
    pub size: Option<ImageSize>,
}
```

### ImageResponse

画像生成レスポンス。

```rust
pub struct ImageResponse {
    /// 生成時刻（Unix timestamp）
    pub created: i64,
    /// 生成された画像データ
    pub data: Vec<ImageData>,
}

pub struct ImageData {
    /// Base64エンコード画像（response_format=b64_jsonの場合）
    pub b64_json: Option<String>,
    /// 画像URL（response_format=urlの場合）
    pub url: Option<String>,
    /// 改訂されたプロンプト
    pub revised_prompt: Option<String>,
}
```

### 列挙型

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageQuality {
    Standard,
    Hd,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageResponseFormat {
    Url,
    B64Json,
}

#[derive(Serialize, Deserialize)]
pub enum ImageSize {
    #[serde(rename = "256x256")]
    Size256,
    #[serde(rename = "512x512")]
    Size512,
    #[serde(rename = "1024x1024")]
    Size1024,
    #[serde(rename = "1792x1024")]
    Size1792x1024,
    #[serde(rename = "1024x1792")]
    Size1024x1792,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageStyle {
    Vivid,
    Natural,
}
```

## RuntimeType拡張

```rust
pub enum RuntimeType {
    LlamaCpp,
    StableDiffusion,  // 画像生成用
    Whisper,
}
```

## ModelType拡張

```rust
pub enum ModelType {
    TextGeneration,
    Embedding,
    ImageGeneration,  // 画像生成用
    SpeechToText,
    TextToSpeech,
}
```
