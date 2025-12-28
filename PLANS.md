# 今後の計画

このファイルは進行中・予定の作業を追跡するためのものです。

## 完了済み

### サブモジュール統一（2025-12-28）

全サードパーティ依存をサブモジュールに統一:

| ライブラリ | リポジトリ (upstream) | バージョン/コミット |
|-----------|----------------------|-------------------|
| llama.cpp | ggerganov/llama.cpp | 特定コミット |
| whisper.cpp | ggerganov/whisper.cpp | 特定コミット |
| stable-diffusion.cpp | leejet/stable-diffusion.cpp | 特定コミット |
| cpp-httplib | yhirose/cpp-httplib | v0.27.0 |
| nlohmann-json | nlohmann/json | v3.12.0 |

### Dependabot設定（2025-12-28）

`.github/dependabot.yml` でサブモジュールの自動更新PRを有効化

- ターゲットブランチ: `develop`
- 更新頻度: weekly
- ラベル: `dependencies`, `submodule`

## 進行中

（現在なし）

## 予定

（現在なし）
