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
- [x] Node: EngineRegistry が format に一致するエンジンのみ解決する
- [x] Tests: EngineRegistry の format フィルタを検証する
- [x] Router: registry manifest に format を明示する
- [x] Node: manifest format を優先して ModelStorage が format を確定する
- [x] Tests: manifest format 優先の contract/unit テストを追加する
- [x] Node: EngineRegistry が capability に一致するエンジンのみ解決する
- [x] Tests: EngineRegistry の capability フィルタを検証する
- [x] Node: InferenceEngine が endpoint に応じた capability でモデルをロードする
- [x] Node: generateEmbeddings で embeddings capability を指定して解決する
- [x] Tests: capability 指定に応じたエンジン解決（loadModel/embeddings）を検証する
- [x] Node: capability未対応のloadModelを明示エラーにする
- [x] Tests: embeddings未対応モデルで400を返すことを検証する
- [x] Node: ModelDescriptor に capabilities を付与し、runtime→capabilities を埋める
- [x] Tests: EngineRegistry のベンチマーク選択とフォールバックを検証する
- [x] Node: ベンチマーク未設定時はプラグイン（非builtin）を優先する
- [ ] DirectML推論パスの実装（演算カーネル、KVキャッシュ、サンプリング）
- [ ] 実GPU環境の統合テスト（小型モデルでE2E）
- [x] 性能/メモリ要件の測定と制約の明文化
