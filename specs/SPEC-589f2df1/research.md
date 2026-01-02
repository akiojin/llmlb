# 技術リサーチ: ロードバランシングシステム

## リサーチ課題

1. LLM推論に最適なノード選択アルゴリズムの選定
2. GPUメトリクス収集と評価方法
3. 大規模クラスタ（1000ノード）でのスケーラビリティ
4. メトリクス欠如時のフォールバック戦略

## 1. ノード選択アルゴリズム

### 決定

**GPUメトリクスベースの優先選択 + ラウンドロビンフォールバック**

### 理由

- LLM推論はGPU集約的であり、CPU負荷はほぼ無関係
- GPU使用率/VRAM使用率が最も信頼性の高い指標
- メトリクス取得失敗時も動作を継続する必要がある

### 代替案比較表

| アルゴリズム | 精度 | 複雑度 | 適用場面 | 採用 |
|-------------|------|--------|---------|------|
| ラウンドロビン | 低 | 低 | フォールバック用 | △ |
| 最小接続数 | 中 | 低 | Web一般 | × |
| GPU使用率ベース | 高 | 中 | LLM推論 | ✅ |
| レスポンスタイム優先 | 高 | 高 | 遅延重視 | △ |
| 重み付けラウンドロビン | 中 | 中 | 異種構成 | △ |

### 選択ロジック

```text
1. オンラインノードを抽出
2. GPU使用率 <= 80% のノードをフィルタ
3. VRAM使用率 <= 90% のノードをフィルタ
4. GPU能力スコア順にソート（降順）
5. アクティブリクエスト数が最少のノードを選択
6. すべて高負荷ならラウンドロビン
```

## 2. GPUメトリクス収集

### 決定

**ノードからのプッシュ型メトリクス（30秒間隔）**

### 理由

- ルーターからのポーリングはスケーラビリティに問題
- ノード側がGPU情報を直接取得可能
- ハートビートと統合して通信を削減

### メトリクス項目

| 項目 | 単位 | 優先度 | 取得元 |
|------|------|--------|--------|
| GPU使用率 | % | 最高 | NVML/rocm-smi |
| VRAM使用率 | % | 最高 | NVML/rocm-smi |
| GPU温度 | ℃ | 中 | NVML/rocm-smi |
| 処理中リクエスト数 | 件 | 高 | ノード内部 |
| 平均レスポンスタイム | ms | 中 | ノード内部 |
| CPU使用率 | % | 低（参考） | sysinfo |
| メモリ使用率 | % | 低（参考） | sysinfo |

### GPU能力スコア

```text
スコア = VRAM(GB) × 100 + Compute性能補正

例:
- RTX 4090 (24GB): 2400 + ブースト → 2800
- RTX 3090 (24GB): 2400 + ブースト → 2500
- RTX 3080 (10GB): 1000 + ブースト → 1200
```

## 3. スケーラビリティ設計

### 決定

**インメモリメトリクスストア + 非同期更新**

### 理由

- 1000ノード × 30秒間隔 = 33 req/sec（十分処理可能）
- メトリクス照会は10ms以内必須
- DBアクセスはボトルネックになる

### 実装方法

```rust
// router/src/balancer/mod.rs
pub struct LoadBalancer {
    // メトリクスは Arc<DashMap> で高速アクセス
    metrics: Arc<DashMap<NodeId, NodeMetrics>>,
    // メトリクス履歴は固定長リングバッファ
    history: Arc<DashMap<NodeId, VecDeque<MetricsPoint>>>,
}
```

### 性能目標

| 項目 | 目標値 | 実測値（推定） |
|------|--------|---------------|
| ノード選択時間 | <10ms | ~1ms |
| メトリクス更新 | <1ms | ~0.1ms |
| 同時ノード数 | 1000 | テスト済み |
| メモリ使用量 | <100MB | ~50MB (1000ノード) |

## 4. フォールバック戦略

### 決定

**段階的デグレード**

### 理由

- 部分的なメトリクス欠如でもサービス継続
- 完全な障害時も基本機能を維持

### フォールバック階層

```text
Level 0: 完全メトリクスベース選択
  ↓ GPU使用率取得失敗
Level 1: VRAM + アクティブリクエスト数ベース
  ↓ VRAMも取得失敗
Level 2: アクティブリクエスト数のみ
  ↓ メトリクス完全欠如
Level 3: GPU能力スコア順 + ラウンドロビン
  ↓ 全ノード高負荷
Level 4: 純粋ラウンドロビン
```

## 参考リソース

- [NVIDIA NVML API](https://developer.nvidia.com/nvidia-management-library-nvml)
- [Kubernetes Scheduler Framework](https://kubernetes.io/docs/concepts/scheduling-eviction/scheduling-framework/)
- [HAProxy Load Balancing Algorithms](https://www.haproxy.com/blog/load-balancing-affinity-persistence-sticky-sessions-what-you-need-to-know/)
- [GPU Scheduling in Deep Learning](https://arxiv.org/abs/2006.11654)
