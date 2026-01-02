# タスク: SPEC-6cd7f960（対応モデルリスト運用）

旧設計（ModelHub/Pull前提）のタスクは無効。新方針に合わせて再計画する。

## 方針更新（2025-12-31）
- `/v0/models/pull` は廃止
- URL登録は維持（`/v0/models/register`）
- Nodeがマニフェストに従ってHFから直接取得

## タスク

### 1) ルール/仕様の整備
- [x] 対応モデルは `supported_models.json` で管理する（Specに明記）
- [x] 追加は「検証済みのみ」の方針を `verified-models.md` に反映
- [x] gpt-oss 20B/120B を Model Hub に追加（Metalアーティファクト確認済み）

### 2) Docker Desktop Models 検証フロー（GGUF）
- [ ] 追加対象（Docker Desktop Models）の **HF repo / GGUFファイル / 量子化** を確定する
  - `specs/SPEC-6cd7f960/verified-models.md` に決定内容を追記
  - 同名モデルが複数ある場合は **最小量子化（Q4/Q5系）優先** で選ぶ
- [ ] `node/third_party/llama.cpp` または `node/build` の `llama-cli` でロード確認
- [ ] テキスト生成の最小スモーク（短文で 1-2 トークン以上）を確認
- [ ] メモリ使用量の記録（`required_memory_bytes` の更新）
- [ ] 検証ログを `specs/SPEC-6cd7f960/verified-models.md` に記録
- [ ] `router/src/supported_models.json` に追加し、Model Hub に反映

### 3) safetensors 検証フロー
- [ ] gpt-oss-safeguard の **Metal最適化アーティファクト有無** を再確認
  - 無い場合は「未検証（Metalアーティファクト無し）」を維持
- [ ] safetensors の検証フロー（Metal）を明文化し、`verified-models.md` に追記
