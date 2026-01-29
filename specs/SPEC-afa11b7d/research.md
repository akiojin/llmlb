# リサーチ: safetensors量子化対応

**機能ID**: `SPEC-afa11b7d`
**作成日**: 2026-01-27
**ステータス**: 調査中

## 現状整理

- xllmはsafetensors形式を`safetensors_cpp`エンジンで実行する
- 量子化指定は現状GGUF向けのみで、safetensorsには適用されない
- safetensors_cppはGPUバックエンド必須（CPU推論は不可）

## safetensors.cppの量子化API実態

- safetensors.cppのC APIは**重み量子化の選択**を受け取らない
- 量子化として外部から制御できるのは**KVキャッシュ量子化**のみ
  - `stcpp_context_params.kv_cache_quant`（bool）
  - `stcpp_context_params.kv_quant_type`（enum）
    - `STCPP_KV_QUANT_INT8`
    - `STCPP_KV_QUANT_FP8`
- したがって本SPECでいう「safetensors量子化」は
  **KVキャッシュ量子化（VRAM最適化）**を指す

## 対応方式（本SPECの確定方針）

### 量子化方式一覧（safetensors）

- `kv_int8`: KVキャッシュをINT8量子化
- `kv_fp8`: KVキャッシュをFP8量子化
- 量子化未指定: 従来どおり非量子化（KV量子化なし）

### バックエンド制約

- safetensors_cppはGPUバックエンド（Metal/CUDA/ROCm/Vulkan）前提
- KVキャッシュ量子化も同じ前提で扱う（CPU向けの特例は設けない）

### 指定の入口（最小構成）

- 量子化指定は**モデル名サフィックス**で受け付ける
  - 例: `model:kv_int8`, `model:kv_fp8`
- 既存のGGUF量子化指定（`model:Q4_K_M`）と共存させる
  - `kv_*`はsafetensors専用の予約トークンとして扱う

## 調査項目

1. **safetensors.cppの量子化対応状況**
   - どの量子化方式に対応しているか
   - 量子化済み重みの読み込み可否
   - 実装に必要な追加アーティファクトの有無

2. **GPUバックエンドごとの制約**
   - Metal/CUDA/ROCm/Vulkanでの対応差
   - 量子化方式ごとの互換性

3. **量子化指定の入力経路**
   - CLI引数/モデル名サフィックス/設定ファイル/APIの優先順位
   - 既存のGGUF指定方式（model:Q4_K_M）との整合

4. **フォールバックとエラー方針**
   - 未対応方式の扱い（明示エラー or フォールバック禁止）
   - 量子化指定とモデルアーキテクチャ不整合の扱い

5. **検証手順と計測**
   - 量子化によるVRAM削減の測定方法
   - 推論品質/性能の受け入れ基準

## 既知の候補・前提

- gpt-oss系モデルはMXFP4などの低精度表現を含む可能性がある（SPEC-69549000の記述に基づく）
- safetensors.cppの公開API上、外部指定できる量子化はKVキャッシュのみ

## 未解決の決定（残タスク）

- 量子化指定時のエラー文言（CLI/API）をどこまで揃えるか
- KV量子化の実計測手順（VRAM差分の受け入れ基準）
