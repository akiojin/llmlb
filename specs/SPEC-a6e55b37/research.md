# 技術リサーチ: llmlb 自動アップデート（Ollama方式）

## 目的
Ollama と同様に OS に応じた更新方法を取り、ユーザー承認後に “停止→入れ替え→再起動” を安全に行う。

## 参考（Ollama）
- macOS: install スクリプトは `/Applications` の入れ替え・`/usr/local/bin` のリンクなど、OS 固有の手順で更新する。
- Linux: systemd を前提にした運用が多く、更新は権限が絡む（root領域への配置）。

## llmlb の現状（本リポジトリ）
- `publish.yml` より、配布物は以下。
  - Unix: `llmlb-<artifact>.tar.gz`
  - Windows: `llmlb-<artifact>.zip`
  - macOS: `llmlb-<artifact>.pkg`（`/usr/local/bin/llmlb` にインストール）
  - Windows: `llmlb-<artifact>-setup.exe`（Inno Setup、`%LOCALAPPDATA%\Programs\llmlb` にインストール）

## 重要な論点
- Windows は実行中 exe の置換が基本的にできないため、別プロセス（内部アップデータ）で “旧PID終了待ち→置換→再起動” が必要。
- macOS pkg は権限昇格が必要になりやすいが、Windows Inno Setup（`PrivilegesRequired=lowest`）は UAC なしでサイレント更新可能。
- `/v1/*` の in-flight を正確に数えないと、クラウドモデル等が drain 判定から漏れる。ミドルウェアで body drop までカウントする。
