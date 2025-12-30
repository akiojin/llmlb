# タスク: SPEC-3fc2c1e4 実行エンジン（統合仕様）

## 方針
- 既存SPECを改修し、重複/矛盾を排除する
- 依存関係マトリクスを更新する
- エンジン領域の実装タスクは **TDD順序（Contract→Integration→E2E→Unit→Core）** を前提に分解する

## Tasks
- [x] 統合仕様の作成（本SPEC）
- [x] 既存SPECの責務境界を明文化
- [x] 依存関係マトリクスの更新
- [x] Node: Engine Host（プラグインローダー）を導入する
- [x] Node: プラグイン ABI/manifest の検証ロジックを整備する
- [x] Node: plugin manifest の gpu_targets に一致しないエンジンをロード対象から除外する
- [x] Node: EngineRegistry で同一runtimeに複数エンジンを登録できるようにする
- [x] Node: ベンチマーク結果に基づいて EngineRegistry が解決する
- [x] Tests: EngineRegistry のベンチマーク選択とフォールバックを検証する
- [ ] DirectML推論パスの実装（演算カーネル、KVキャッシュ、サンプリング）
- [ ] 実GPU環境の統合テスト（小型モデルでE2E）
- [ ] 性能/メモリ要件の測定と制約の明文化
