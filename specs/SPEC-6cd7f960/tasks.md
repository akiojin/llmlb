# タスク: SPEC-6cd7f960 対応モデルリスト型管理

## 方針
- 対応モデルは静的リストで管理する
- ルーターはバイナリを保持せず、Node が HF から直接取得する
- UI は Model Hub（対応モデル）と Local（登録済み）の2タブ

## Tasks

### Router
- [x] 対応モデルリストを JSON で定義する
- [x] `/v0/models/hub` で対応モデル + 状態を返す（available/registered/ready）
- [x] HF動的情報（downloads/likes）をキャッシュ付きで付与する
- [x] `/v0/models/register` を維持し、pull/ダウンロード系APIを廃止する

### Node
- [x] マニフェスト参照 + HF 直取得の動線を維持する

### Dashboard
- [x] Model Hub タブに Register 導線を提供する
- [x] Local タブで登録済みモデルの状態を表示する

### Tests
- [x] Model Hub API（/v0/models/hub）の一覧と状態を検証する
- [x] Dashboard の Model Hub 表示を検証する

### Docs
- [x] 仕様/計画/タスクの再整理（本SPEC）
