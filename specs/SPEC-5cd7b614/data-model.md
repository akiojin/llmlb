# データモデル: GPU必須ノード登録要件

## エンティティ定義

### GpuDevice

個別のGPUデバイス情報。

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDevice {
    /// GPUモデル名
    pub model: String,
    /// デバイス数
    pub count: u32,
    /// ベンダー（nvidia, amd, intel, apple）
    pub vendor: GpuVendor,
}
```

### GpuVendor

GPUベンダー。

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Unknown,
}
```

### GpuCapability

GPU能力情報（NVIDIA GPU専用）。

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuCapability {
    /// GPU能力スコア（0-10000）
    pub score: u32,
    /// Compute Capability（例: 8.6）
    pub compute_capability: String,
    /// VRAMサイズ（GB）
    pub vram_gb: f32,
    /// 最大クロック速度（MHz）
    pub max_clock_mhz: u32,
}
```

### NodeRegistration拡張

```rust
pub struct NodeRegistration {
    pub runtime_id: Uuid,
    pub node_ip: String,
    pub port: u16,
    /// GPU情報（必須、空配列は拒否）
    pub gpu_devices: Vec<GpuDevice>,
    /// GPU能力情報（オプション、NVIDIA GPUのみ）
    pub gpu_capability: Option<GpuCapability>,
}
```

## API仕様

### 登録成功レスポンス

```json
{
  "runtime_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "registered",
  "gpu_devices": [
    {"model": "NVIDIA GeForce RTX 4090", "count": 1, "vendor": "nvidia"}
  ],
  "gpu_capability": {
    "score": 9850,
    "compute_capability": "8.9",
    "vram_gb": 24.0,
    "max_clock_mhz": 2520
  }
}
```

### 登録拒否レスポンス（403）

```json
{
  "error": {
    "message": "GPU hardware is required for node registration",
    "type": "forbidden",
    "code": "gpu_required"
  }
}
```

## バリデーションルール

### gpu_devicesフィールド

| 条件 | 結果 |
|------|------|
| フィールドなし | 403 Forbidden |
| 空配列 `[]` | 403 Forbidden |
| count = 0 | 403 Forbidden |
| count > 0 | 登録許可 |
| model = "" | 登録許可（「不明」として表示） |

### GPU能力スコア計算

```rust
fn calculate_gpu_score(
    vram_gb: f32,
    max_clock_mhz: u32,
    compute_capability: f32,
) -> u32 {
    // 各要素に重み付けしてスコア計算
    let vram_score = (vram_gb * 100.0) as u32;      // 最大2400点
    let clock_score = max_clock_mhz / 3;            // 最大約800点
    let compute_score = (compute_capability * 500.0) as u32; // 最大約4500点

    (vram_score + clock_score + compute_score).min(10000)
}
```

## データベーススキーマ

### nodesテーブル拡張

```sql
ALTER TABLE nodes ADD COLUMN gpu_devices TEXT;
ALTER TABLE nodes ADD COLUMN gpu_capability_score INTEGER;
ALTER TABLE nodes ADD COLUMN gpu_compute_capability TEXT;
```

### GPU情報の永続化形式

```json
{
  "gpu_devices": "[{\"model\":\"NVIDIA RTX 4090\",\"count\":1,\"vendor\":\"nvidia\"}]",
  "gpu_capability_score": 9850,
  "gpu_compute_capability": "8.9"
}
```

## 起動時クリーンアップ

### 削除対象条件

```rust
fn should_delete_node(node: &Node) -> bool {
    node.gpu_devices.is_none()
        || node.gpu_devices.as_ref().map(|g| g.is_empty()).unwrap_or(true)
}
```

### クリーンアップログ

```text
[INFO] Removed 3 nodes without GPU information during startup cleanup
```
