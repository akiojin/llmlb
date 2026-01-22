# データモデル: 画像認識モデル対応（Image Understanding）

## エンティティ定義

### ImageContent

```rust
/// 画像コンテンツ
pub enum ImageContent {
    /// URL形式
    Url {
        /// 画像URL
        url: String,
        /// 詳細レベル（OpenAI互換、現在は無視）
        detail: Option<ImageDetail>,
    },
    /// Base64形式
    Base64 {
        /// Base64エンコードされた画像データ
        data: String,
        /// MIMEタイプ
        media_type: ImageMediaType,
    },
}

#[derive(Clone, Copy)]
pub enum ImageDetail {
    Auto,
    Low,
    High,
}

#[derive(Clone, Copy)]
pub enum ImageMediaType {
    Jpeg,
    Png,
    Gif,
    WebP,
}
```

### VisionRequest

```rust
/// Vision対応チャットリクエスト
pub struct VisionChatRequest {
    /// モデルID
    pub model: String,
    /// メッセージ一覧
    pub messages: Vec<VisionMessage>,
    /// ストリーミング有効化
    pub stream: bool,
    /// 最大生成トークン数
    pub max_tokens: Option<u32>,
    /// 温度パラメータ
    pub temperature: Option<f32>,
}

pub struct VisionMessage {
    pub role: MessageRole,
    pub content: VisionContent,
}

pub enum VisionContent {
    /// テキストのみ
    Text(String),
    /// マルチモーダル（テキスト + 画像）
    MultiModal(Vec<ContentPart>),
}

pub enum ContentPart {
    Text { text: String },
    Image { image: ImageContent },
}
```

### DecodedImage

```rust
/// デコード済み画像（ロードバランサーでの処理後）
pub struct DecodedImage {
    /// 生画像データ
    pub data: Vec<u8>,
    /// 画像形式
    pub format: ImageMediaType,
    /// 幅（ピクセル）
    pub width: u32,
    /// 高さ（ピクセル）
    pub height: u32,
    /// ファイルサイズ（バイト）
    pub size: usize,
}
```

### VisionCapability

```rust
/// モデルのVision対応情報
pub struct VisionCapability {
    /// Vision機能の対応有無
    pub supported: bool,
    /// 対応画像形式
    pub supported_formats: Vec<ImageMediaType>,
    /// 1リクエストあたりの最大画像数
    pub max_images_per_request: u32,
    /// 最大画像サイズ（バイト）
    pub max_image_size: usize,
    /// 推奨画像解像度
    pub recommended_resolution: Option<(u32, u32)>,
}

impl Default for VisionCapability {
    fn default() -> Self {
        Self {
            supported: false,
            supported_formats: vec![],
            max_images_per_request: 0,
            max_image_size: 0,
            recommended_resolution: None,
        }
    }
}
```

### ImageError

```rust
/// 画像処理エラー
pub enum ImageError {
    /// 画像が大きすぎる
    TooLarge {
        actual: usize,
        max: usize,
    },
    /// 画像数が多すぎる
    TooManyImages {
        actual: usize,
        max: usize,
    },
    /// サポートされていない形式
    UnsupportedFormat {
        format: String,
        supported: Vec<String>,
    },
    /// Base64デコード失敗
    InvalidBase64 {
        reason: String,
    },
    /// URL取得失敗
    FetchFailed {
        url: String,
        reason: String,
    },
    /// URL取得タイムアウト
    FetchTimeout {
        url: String,
        timeout_secs: u64,
    },
    /// 画像データ破損
    CorruptedImage {
        reason: String,
    },
    /// モデルがVision非対応
    ModelNotSupported {
        model_id: String,
    },
}
```

### VisionConfig

```rust
/// Vision機能設定
pub struct VisionConfig {
    /// 最大画像サイズ（バイト、デフォルト: 10MB）
    pub max_image_size: usize,
    /// 1リクエストあたりの最大画像数（デフォルト: 10）
    pub max_images_per_request: u32,
    /// URL取得タイムアウト（秒、デフォルト: 30）
    pub fetch_timeout_secs: u64,
    /// リダイレクト最大回数（デフォルト: 3）
    pub max_redirects: u32,
    /// 対応画像形式
    pub supported_formats: Vec<ImageMediaType>,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            max_image_size: 10 * 1024 * 1024, // 10MB
            max_images_per_request: 10,
            fetch_timeout_secs: 30,
            max_redirects: 3,
            supported_formats: vec![
                ImageMediaType::Jpeg,
                ImageMediaType::Png,
                ImageMediaType::Gif,
                ImageMediaType::WebP,
            ],
        }
    }
}
```

## 検証ルール

| エンティティ | フィールド | ルール |
|-------------|-----------|--------|
| ImageContent::Url | url | 有効なHTTP/HTTPS URLであること |
| ImageContent::Base64 | data | 有効なBase64文字列であること |
| DecodedImage | size | max_image_size以下であること |
| DecodedImage | format | supported_formatsに含まれること |
| VisionChatRequest | messages | 1つ以上のメッセージを含むこと |
| VisionChatRequest | messages | 画像総数がmax_images_per_request以下 |
| VisionCapability | supported | trueの場合、supported_formatsが空でないこと |

## 関係図

```text
┌─────────────────────────────────────────────────────────────────┐
│                    VisionChatRequest                             │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                   messages[]                             │    │
│  │  ┌───────────────────────────────────────────────────┐  │    │
│  │  │              VisionMessage                         │  │    │
│  │  │  ┌───────────────────────────────────────────┐    │  │    │
│  │  │  │         VisionContent::MultiModal          │    │  │    │
│  │  │  │  ┌─────────────┐ ┌─────────────────────┐  │    │  │    │
│  │  │  │  │ ContentPart │ │    ContentPart      │  │    │  │    │
│  │  │  │  │ ::Text      │ │    ::Image          │  │    │  │    │
│  │  │  │  └─────────────┘ └──────────┬──────────┘  │    │  │    │
│  │  │  └─────────────────────────────│─────────────┘    │  │    │
│  │  └────────────────────────────────│──────────────────┘  │    │
│  └───────────────────────────────────│─────────────────────┘    │
└──────────────────────────────────────│──────────────────────────┘
                                       │
                                       ▼
                            ┌──────────────────────┐
                            │    ImageContent      │
                            │  ┌────────────────┐  │
                            │  │ Url { url }    │  │
                            │  └────────────────┘  │
                            │  ┌────────────────┐  │
                            │  │ Base64 { data }│  │
                            │  └────────────────┘  │
                            └──────────┬───────────┘
                                       │ decode
                                       ▼
                            ┌──────────────────────┐
                            │    DecodedImage      │
                            │  - data: Vec<u8>     │
                            │  - format            │
                            │  - width, height     │
                            │  - size              │
                            └──────────────────────┘

                    Model Capability Check
                            ┌──────────────────────┐
                            │   VisionCapability   │
                            │  - supported         │
                            │  - supported_formats │
                            │  - max_images        │
                            │  - max_size          │
                            └──────────────────────┘
```

## /v1/models レスポンス拡張

```json
{
  "id": "llava-1.6-7b",
  "object": "model",
  "created": 1700000000,
  "owned_by": "lb",
  "capabilities": {
    "text_generation": true,
    "image_understanding": true,
    "image_generation": false,
    "speech_to_text": false,
    "text_to_speech": false
  }
}
```
