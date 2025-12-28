# 今後の計画

このファイルは進行中・予定の作業を追跡するためのものです。

## 完了済み

### サブモジュール統一（2025-12-28）

全サードパーティ依存をサブモジュールに統一:

| ライブラリ | バージョン | リポジトリ |
|-----------|-----------|-----------|
| llama.cpp | upstream | ggerganov/llama.cpp |
| whisper.cpp | upstream | ggerganov/whisper.cpp |
| stable-diffusion.cpp | upstream | leejet/stable-diffusion.cpp |
| cpp-httplib | v0.27.0 | yhirose/cpp-httplib |
| nlohmann-json | v3.12.0 | nlohmann/json |

### Dependabot設定（2025-12-28）

`.github/dependabot.yml` でサブモジュールの自動更新PRを有効化

- ターゲットブランチ: `develop`
- 更新頻度: weekly
- ラベル: `dependencies`, `submodule`

## 進行中

（現在なし）

## 予定

（現在なし）
