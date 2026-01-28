# タスク: SPEC-6cd7f960（対応モデルリスト運用）

**ステータス**: 完了（自動認識への移行 2026-01-27完了）

~~旧設計（ModelHub/Pull前提）のタスクは無効。新方針に合わせて再計画する。~~

## 方針（廃止 2026-01-25）
~~- 対応モデルは静的リストで管理する~~
- ロードバランサーはバイナリを保持せず、Node が HF から直接取得する
~~- UI は Model Hub（対応モデル）と Local（登録済み）の2タブ~~

## 方針更新（2025-12-31）
- `/v0/models/pull` は廃止
- URL登録は維持（`/v0/models/register`）
- Nodeがマニフェストに従ってHFから直接取得

## 方針更新（2026-01-25）- **`supported_models.json` を完全廃止**
- ロードバランサーは各エンドポイントの `/v1/models` を集約するのみ
- モデルアーキテクチャ認識はエンドポイント（xLLM）側で行う
- 任意のHuggingFaceモデルを登録可能に変更

## 完了済み（初期実装）
- 対応モデルリストを JSON で定義
- `/v0/models/hub` で対応モデル + 状態を返す（available/registered/ready）
- HF動的情報（downloads/likes）をキャッシュ付きで付与
- `/v0/models/register` を維持し、pull/ダウンロード系APIを廃止
- マニフェスト参照 + HF 直取得の動線を維持
- Model Hub タブに Register 導線を提供
- Local タブで登録済みモデルの状態を表示
- Model Hub API（/v0/models/hub）の一覧と状態を検証
- Dashboard の Model Hub 表示を検証
- 仕様/計画/タスクの再整理（本SPEC）

## タスク

### 1) ルール/仕様の整備
- [x] 対応モデルは `supported_models.json` で管理する（Specに明記）
- [x] 追加は「検証済みのみ」の方針を `verified-models.md` に反映
- [x] gpt-oss 20B/120B を Model Hub に追加（Metalアーティファクト確認済み）

### 2) Docker Desktop Models 検証フロー（GGUF）
- [x] 追加対象（Docker Desktop Models）の **HF repo / GGUFファイル / 量子化** を確定する
  - `specs/SPEC-6cd7f960/verified-models.md` に決定内容を追記
  - 同名モデルが複数ある場合は **最小量子化（Q4/Q5系）優先** で選ぶ
- [x] `node/third_party/llama.cpp` または `node/build` の `llama-cli` でロード確認
- [x] テキスト生成の最小スモーク（短文で 1-2 トークン以上）を確認
- [x] メモリ使用量の記録（`required_memory_bytes` の更新）
- [x] 検証ログを `specs/SPEC-6cd7f960/verified-models.md` に記録
- [x] `llmlb/src/supported_models.json` に追加し、Model Hub に反映

**完了ステータス（2026-01-05）:**
アクセス可能な全Docker Desktop Modelsの検証を完了。以下のモデルは外部要因でブロック:
- kimi-k2: GGUF分割・大容量（13ファイル）
- granite-4.0-nano/h-nano: HFアクセス不可
- gemma3-qat: HF gated
- deepcoder-preview: HFアクセス不可

### 3) safetensors 検証フロー
- [x] gpt-oss-safeguard の **Metal最適化アーティファクト有無** を再確認
  - 無い場合は「未検証（Metalアーティファクト無し）」または「HF未公開」を維持
- [x] safetensors の検証フロー（Metal）を明文化し、`verified-models.md` に追記

## Phase 4: 自動認識への移行（2026-01-25追加）

### 廃止対応

- [x] 4.1 `llmlb/src/supported_models.json` を削除
- [x] 4.2 `REGISTERED_MODELS` 定数を削除（`SUPPORTED_MODELS_JSON`, `SupportedModel`, `load_supported_models()`）
- [x] 4.3 `/v0/models/hub` APIを登録済みモデルのみ返すよう変更
- [x] 4.4 Model Hub タブをUIから削除（ModelHubTab.tsx削除、ModelsSection.tsxからModel Hubタブ削除）

### Core

- [x] 4.5 `/v1/models` をエンドポイント集約のみに変更
  - 登録済みだがオンラインエンドポイントにないモデルは/v1/modelsに含めない（FR-6準拠）
  - openai.rsから登録済みモデル追加部分を削除
- [x] 4.6 モデル登録フローからsupported_models.jsonチェックを削除（登録は任意のHFモデル対応済み）

### Test

- [x] [P] 4.7 Unit Test: エンドポイント集約動作
  - v1_models_aggregates_multiple_endpoints（複数エンドポイントのモデル集約）
  - v1_models_excludes_models_not_on_endpoints（エンドポイントにないモデルは除外）
- [x] [P] 4.8 Integration Test: 任意のHFモデル登録
  - test_register_model_contract（任意のHFリポジトリ登録、既存テストで対応済み）
