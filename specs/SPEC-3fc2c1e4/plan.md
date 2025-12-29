# SPEC-3fc2c1e4: Plan

**仕様**: [spec.md](./spec.md)

## 目的
実行エンジン領域の要件を統合し、モデル管理との境界を明確化する。

## 方針
- 既存SPECを「詳細仕様」として参照し、重複/矛盾を排除する
- 依存関係マトリクスを更新する
- NemotronエンジンはTBDとして後回しにする
- 内蔵エンジンはプラグイン形式（動的ロード）で統一する

## 更新対象
- `specs/specs.md`（カテゴリ分割/依存関係）
- `SPEC-d7feaa2c`, `SPEC-2c0e5a9b`, `SPEC-efff1da7`

## 進め方
1. 既存SPECの重複/矛盾点を抽出
2. 本SPECに原則/責務/非ゴールを明示
3. 依存関係を整理し一覧に反映

## 矛盾/漏れ一覧（2025-12-24 時点）

- **GPU対象範囲の不一致（対応済み）**  
  - 決定事項: macOSはApple Silicon/Metal、WindowsはDirectML、Linuxは非対応（CUDAは実験扱い）、WSL2は対象外。  
  - `SPEC-5cd7b614` は AMD/Intel を含む前提、Docker for Mac を含むため差分がある。
- **モデルストレージ設計の古い前提（対応済み）**  
  - `SPEC-dcaeaec4/plan.md` に `metadata.json` の記載と GGUF中心のディレクトリ構造が残っている。
  - 決定事項: `metadata.json` は使用しない、safetensors優先。
- **キャッシュ再処理の表現（対応済み）**  
  - `SPEC-6c2d9f1e` の FR-002 に「再変換」が残っている。
  - 決定事項: 自動変換は禁止。再取得（再ダウンロード）のみ。
- **音声エンジンの不一致（対応済み）**  
  - `SPEC-26006000` / `SPEC-6c2d9f1e` が whisper.cpp/ONNX 前提。
  - 決定事項: ASRはwhisper.cpp（GGUF運用）、TTSはONNX Runtime、Python依存なし。
- **画像生成エンジンの不一致（対応済み / 要調査あり）**  
  - `SPEC-ae3f974e` が stable-diffusion.cpp + GGML/GGUF 前提。
  - 決定事項: safetensors を正本とし、GGUFは存在しない場合のみフォールバック。  
  - stable-diffusion.cpp の safetensors 直接対応可否を明確化する必要あり。
- **画像認識エンジンの未確定（対応済み）**  
  - `SPEC-e03a404c` は llama.cpp 等を示唆しているが、新エンジン方針と整合が取れていない。
- **モデルソース種別の不足（対応済み）**  
  - `SPEC-47649000` に `hf_safetensors` のソース種別が未記載。
- **chat_template(Jinja)完全互換の欠落（未対応）**  
  - 完全互換要件は会話で合意済みだが、独立SPECが未作成（`SPEC-d7feaa2c` に保留として残存）。
- **GGUFアーキテクチャ仕様の位置づけ（対応済み）**  
  - `SPEC-8a2d1d43` が GGUF 前提。safetensors主経路との関係（GGUFフォールバック専用）を明記する必要がある。
