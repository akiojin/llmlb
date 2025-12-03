# データモデル: SPEC-11106000 Hugging Face GGUFモデル対応登録

## エンティティ

### ModelInfo（拡張）
- `name`: String — 対応モデルID（例: `hf/TheBloke/Llama-2-7B-GGUF/llama-2-7b.Q4_K_M.gguf`）
- `source`: Enum { `predefined`, `hf_gguf`, `hf_pending_conversion` }
- `display_name`: String — UI/CLI表示用（例: `Llama-2-7B Q4_K_M (TheBloke)`）
- `size_bytes`: u64 — ダウンロードサイズ
- `required_memory_bytes`: u64? — 推奨メモリ（あれば）
- `tags`: Vec<String> — 用途・量子化ラベル
- `download_url`: String — GGUF直接URL（HF raw もしくは内部ストレージ）
- `repo`: String — HFリポジトリ名
- `filename`: String — GGUFファイル名
- `last_modified`: DateTime? — HF最終更新
- `status`: Enum { `available`, `registered`, `downloaded` } — カタログ上の状態
- `notes`: String? — 警告/補足（容量超過など）

### AvailableModelView（HFカタログ用）
- `models`: [ModelInfo-like] — カタログエントリ
- `source`: String — `"hf"` など
- `pagination`: { `limit`, `offset`, `total` }?
- `cached`: bool — キャッシュ使用フラグ

### DownloadTask（既存拡張）
- `task_id`: UUID
- `model_name`: String
- `target`: Enum { `all`, `specific` }
- `node_ids`: Vec<UUID>
- `status`: Enum { `pending`, `downloading`, `completed`, `failed` }
- `progress`: f32 (0–1)
- `speed_bps`: u64?
- `error`: String?

## 関係
- ModelInfo は router の対応モデルリストに格納され、/v1/models で exposed。
- AvailableModelView は /api/models/available で HF カタログを提供。
- DownloadTask はモデル配布・ダウンロード進行を表し、ノードのタスク更新と紐づく。

## バリデーション
- `name` は一意。`hf/` プレフィックス必須（hf系）。
- `download_url` は https であること。
- `size_bytes` が 0 または未設定の場合、警告を返す。
- ノードダウンロード時、`required_memory_bytes` がノード GPU メモリを超える場合は警告を付与。
