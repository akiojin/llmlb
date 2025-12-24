# 機能仕様書: モデル形式選択（safetensors/GGUF）とGGUF選択ポリシー

**機能ID**: `SPEC-a61b24f2`  
**作成日**: 2025-12-21  
**ステータス**: 実装済み  
**入力**: ユーザー説明: "HF上に safetensors/GGUF のいずれもある場合は登録時に必ず選択する。GGUFを選ぶ場合は登録時に品質/省メモリ/速度から量子化モデルを選択できるようにし、ダッシュボードにも説明表示が必要。READMEにも記載。"

## ユーザーシナリオ＆テスト *(必須)*

### ユーザーストーリー1 - 形式選択（safetensors/GGUF） (P1)
運用管理者として、Hugging Face 上に safetensors と GGUF の両方が存在する場合、登録時にどちらを使うか明示的に選択したい。選択が無い場合は明確なエラーを受け取りたい。

**独立テスト**: safetensors と GGUF が両方存在するリポジトリを `format` 未指定で登録 → 400 エラー（「format が必須」）。

### ユーザーストーリー2 - GGUF選択ポリシー（品質/省メモリ/速度） (P1)
運用管理者として、GGUFを選択した場合に、品質/省メモリ/速度の観点で最適な量子化GGUFを登録時に選びたい。リポジトリ内に該当が無い場合は明確なエラーを受け取りたい。

**独立テスト**: `format=gguf` + `filename` 未指定 + `gguf_policy=quality` で登録 → siblings から最適なGGUFを選択 → 登録成功。該当GGUFが無い場合は 400 エラー。

### ユーザーストーリー3 - safetensorsを正として登録 (P1)
運用管理者として、`format=safetensors` を選択した場合は safetensors（必要に応じて index + shards）を正として登録したい。必要なメタデータ（`config.json`, `tokenizer.json`）が無い場合は明確なエラーを受け取りたい。

### エッジケース
- `format` が不正な値の場合、400エラー。
- `format=gguf` で `gguf_policy` が不正な値の場合、400エラー。
- `format=safetensors` で `config.json` または `tokenizer.json` が存在しない場合は 400 エラー。

## 要件 *(必須)*

### 機能要件
- **FR-001**: `/v0/models/register` に `format`（`safetensors`/`gguf`）を追加できる。
- **FR-002**: HF上に safetensors と GGUF が両方ある場合、`format` 未指定は 400 エラー。
- **FR-003**: `format=gguf` かつ `filename` 未指定の場合、`gguf_policy` を必須とし、siblings から最適なGGUFを選択する（見つからない場合は 400）。
- **FR-004**: `format=safetensors` の登録では、`config.json` と `tokenizer.json` を必須とする（不足時は 400）。
- **FR-005**: ダッシュボードの登録ダイアログで `format` と `gguf_policy` を選択でき、説明が表示される（`gguf_policy` は `format=gguf` の場合のみ表示）。
- **FR-006**: README.md / README.ja.md に形式選択とGGUF選択ポリシー、必要な外部ツールの説明を追記する。

### 主要エンティティ
- **形式選択（format）**: 登録時に `safetensors` または `gguf` を選ぶ。
- **GGUF選択ポリシー（gguf_policy）**: `quality` / `memory` / `speed` のいずれか。
- **GGUF siblings**: HFモデルリポジトリ内ファイル一覧（`expand=siblings` の rfilename）。

## スコープ外
- 量子化の自動品質評価（PPL/KLD 等）。
- 量子化タイプの自動最適化。
- 非GGUF入力からの自動変換、および量子化GGUFの自動生成。
- HF URL 入力/登録フローの詳細（SPEC-11106000）。

## 技術制約
- GGUFの選択はHFの既存siblingsから行う（該当が無い場合はエラー）。

## 依存関係
- SPEC-08d2b908（モデル管理統合）

## 成功基準 *(必須)*
1. safetensors/GGUFが両方ある場合に `format` が必須となり、登録時に選択できる。
2. GGUF選択ポリシーで siblings から最適なGGUFを選択できる。
3. ダッシュボード上で `format` と `gguf_policy` の選択と説明が表示される。
4. README.md / README.ja.md に利用方法が記載される。
