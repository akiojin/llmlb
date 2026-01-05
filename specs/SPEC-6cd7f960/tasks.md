# タスク: SPEC-6cd7f960（対応モデルリスト運用）

旧設計（ModelHub/Pull前提）のタスクは無効。新方針に合わせて再計画する。

## 方針
- 対応モデルは静的リストで管理する
- ルーターはバイナリを保持せず、Node が HF から直接取得する
- UI は Model Hub（対応モデル）と Local（登録済み）の2タブ

## 方針更新（2025-12-31）
- `/v0/models/pull` は廃止
- URL登録は維持（`/v0/models/register`）
- Nodeがマニフェストに従ってHFから直接取得

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
- [ ] 追加対象（Docker Desktop Models）の **HF repo / GGUFファイル / 量子化** を確定する
  - `specs/SPEC-6cd7f960/verified-models.md` に決定内容を追記
  - 同名モデルが複数ある場合は **最小量子化（Q4/Q5系）優先** で選ぶ
- [ ] `node/third_party/llama.cpp` または `node/build` の `llama-cli` でロード確認
- [ ] テキスト生成の最小スモーク（短文で 1-2 トークン以上）を確認
- [ ] メモリ使用量の記録（`required_memory_bytes` の更新）
- [ ] 検証ログを `specs/SPEC-6cd7f960/verified-models.md` に記録
- [ ] `router/src/supported_models.json` に追加し、Model Hub に反映

### 3) safetensors 検証フロー
- [x] gpt-oss-safeguard の **Metal最適化アーティファクト有無** を再確認
  - 無い場合は「未検証（Metalアーティファクト無し）」または「HF未公開」を維持
- [x] safetensors の検証フロー（Metal）を明文化し、`verified-models.md` に追記
