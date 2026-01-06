# SPEC-6cd7f960: 実装計画

**ステータス**: ✅ 実装完了

## 方針
- 対応モデルは静的リスト（内部）で管理する
- 登録はメタデータのみ（ルーターはバイナリ非保持）
- Node がマニフェストに従って HF から直接取得する
- ダッシュボードは「Model Hub（対応モデル）」と「Local（登録済み）」の2タブ構成

## 実装内容（完了）

### Router
- 対応モデルの静的定義（JSON）
- `/v0/models/hub` で対応モデル一覧 + 状態（available/registered/ready）を返却
- HF動的情報（downloads/likes）はキャッシュ付きで付与
- `/v0/models/register` は維持（メタデータのみ保存）

### Node
- マニフェスト参照で HF から直接取得（ルーターはバイナリ非保持）

### Dashboard
- Model Hub タブに「Register」導線
- Local タブに登録済みモデルの状態表示

## 完了条件
- 対応モデル一覧の表示と登録が一通り動作する
- Router がバイナリを保持せず、Node が直接取得する
