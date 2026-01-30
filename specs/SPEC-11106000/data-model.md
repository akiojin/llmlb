# データモデル: SPEC-11106000 Hugging Face URL 登録（変換なし）

## エンティティ

### ModelInfo（拡張）
- `name`: String — 対応モデルID（例: `hf/org/repo` または `hf/org/repo/model.safetensors`）
- `source`: Enum { `predefined`, `hf` }
- `display_name`: String — UI/CLI表示用（例: `Llama-2-7B (TheBloke)`）
- `repo`: String — HFリポジトリ名
- `filename`: String? — ファイルURL登録時の対象ファイル名（repo登録時はnull）
- `revision`: String? — HFのrevision/commit（必要時）
- `artifacts`: Vec<ModelArtifact> — Node向けマニフェスト（ファイル一覧）
- `size_bytes`: u64? — 合計サイズ（任意）
- `tags`: Vec<String>? — 用途・量子化ラベル（任意）
- `last_modified`: DateTime? — HF最終更新（任意）

### ModelArtifact（新規）
- `filename`: String — ファイル名（HF repo内のパス）
- `format`: Enum { `safetensors`, `gguf`, `metal`, `onnx`, `other` }
- `size_bytes`: u64? — ファイルサイズ（任意）
- `sha256`: String? — 署名/検証用（任意）
- `notes`: String? — 補足（例: `requires_metal`, `quant=q4_k_m`）

## 関係
- ModelInfo は router の対応モデルリストに格納され、/v1/models で外部公開される。
- Node は `/api/models/registry/:model_name/manifest.json` から ModelArtifact を取得し、
  自身のruntime/GPU要件に合うファイルをHFから直接ダウンロードする。

## バリデーション
- `name` は一意。`hf/` プレフィックス必須（hf系）。
- `repo` は必須。
- `filename` が指定された場合、`artifacts` 内の一致ファイルが存在すること。
- `artifacts` は対応形式（safetensors/gguf/metal等）のいずれかを含むこと。
- サイズやハッシュが未取得の場合は null を許可（Node側で検証する）。
