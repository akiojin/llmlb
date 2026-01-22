# 機能仕様一覧

> 最終更新: 2026-01-18 (SPEC-66555000実装状態更新)
>
> 総SPEC数: **49** | 廃止: 6 | plan.md欠損: 0 | 依存関係完全化済み

**凡例:**

- **Status**: ✅ plan.md有り | 📋 plan.md無し | 🗑️ 廃止
- **Priority**: P1(最優先) | P2(重要) | P3(通常) | -（廃止/対象外）
- **実装**: ✅ 完了 | 🔨 実装中 | 📝 未着手

## 依存関係マトリクス

> 49件中、明示的な依存関係があるSPEC: 33件 | 基盤SPEC（依存なし）: 10件 | 廃止: 6件

| SPEC ID | 依存先 | 備考 |
|---------|--------|------|
| `SPEC-026b2cde` | SPEC-fbc50d97 | ページネーション |
| `SPEC-24157000` | SPEC-63acef08 | OpenAI互換API完全準拠 |
| `SPEC-05098000` | SPEC-589f2df1, SPEC-63acef08 | キューイング |
| `SPEC-08d2b908` | SPEC-dcaeaec4, SPEC-11106000, SPEC-48678000, SPEC-6c2d9f1e | 統合仕様 |
| `SPEC-0c4f3e5c` | - | 🗑️ 廃止（→SPEC-dcaeaec4） |
| `SPEC-11106000` | SPEC-dcaeaec4 | HF登録 |
| `SPEC-1970e39f` | SPEC-799b8e2b | ロギング |
| `SPEC-1f2a9c3d` | SPEC-799b8e2b | ログAPI |
| `SPEC-26006000` | SPEC-dcaeaec4 | 音声対応 |
| `SPEC-2c0e5a9b` | SPEC-3fc2c1e4, SPEC-d7feaa2c, SPEC-08d2b908, SPEC-11106000 | gpt-oss実行 |
| `SPEC-32637000` | SPEC-6c2d9f1e | ルーティング |
| `SPEC-32e2b31a` | - | アーカイブ |
| `SPEC-35375000` | - | 基盤機能 |
| `SPEC-3df1b977` | - | 🗑️ 廃止（→SPEC-48678000） |
| `SPEC-3fc2c1e4` | SPEC-d7feaa2c, SPEC-efff1da7 | 統合仕様 |
| `SPEC-443acc8c` | - | 🗑️ 廃止（→SPEC-66555000） |
| `SPEC-47649000` | - | 基盤機能 |
| `SPEC-47c6f44c` | - | CI/CD |
| `SPEC-48678000` | SPEC-11106000, SPEC-dcaeaec4, SPEC-6cd7f960 | モデル解決 |
| `SPEC-4b6e9f2a` | SPEC-63acef08 | クラウドルーティング |
| `SPEC-589f2df1` | SPEC-63acef08 | ロードバランシング |
| `SPEC-5cd7b614` | - | 基盤機能 |
| `SPEC-5fc9fe92` | SPEC-712c20cf | Playground |
| `SPEC-63acef08` | SPEC-66555000 | 統一APIプロキシ |
| `SPEC-66555000` | SPEC-712c20cf, SPEC-63acef08 | **新規**: エンドポイント登録 |
| `SPEC-69549000` | SPEC-d7feaa2c | safetensors.cpp |
| `SPEC-6c2d9f1e` | SPEC-11106000, SPEC-26006000, SPEC-32637000 | モデル登録 |
| `SPEC-6cd7f960` | SPEC-11106000, SPEC-dcaeaec4, SPEC-d4eb8796, SPEC-66555000 | モデルリスト |
| `SPEC-712c20cf` | SPEC-66555000, SPEC-63acef08, SPEC-d4eb8796 | ダッシュボード |
| `SPEC-799b8e2b` | - | 基盤機能 |
| `SPEC-7c0a37e0` | SPEC-d4eb8796 | APIキースコープ |
| `SPEC-82491000` | SPEC-4b6e9f2a | クラウド統合 |
| `SPEC-83825900` | SPEC-efff1da7, SPEC-d7feaa2c | PoC |
| `SPEC-8a2d1d43` | - | 基盤機能 |
| `SPEC-93536000` | SPEC-dcaeaec4, SPEC-05098000 | ノードベースモデル管理 |
| `SPEC-94621a1f` | - | 🗑️ 廃止（→SPEC-66555000） |
| `SPEC-a61b24f2` | - | 🗑️ 廃止（統合仕様へ移行） |
| `SPEC-a7e6d40a` | - | CLI |
| `SPEC-ae3f974e` | SPEC-dcaeaec4 | 画像生成 |
| `SPEC-d4eb8796` | SPEC-66555000 | 認証 |
| `SPEC-d7feaa2c` | - | **基盤**: エンジン |
| `SPEC-dc648675` | - | CI/CD |
| `SPEC-dcaeaec4` | - | **基盤**: ストレージ |
| `SPEC-e03a404c` | SPEC-6c2d9f1e, SPEC-47649000 | 画像認識 |
| `SPEC-ea015fbb` | SPEC-712c20cf, SPEC-d4eb8796, SPEC-fbc50d97, SPEC-5fc9fe92 | UI索引 |
| `SPEC-ee2aa3ef` | SPEC-47c6f44c | CI/CD |
| `SPEC-efff1da7` | - | PoC |
| `SPEC-fbc50d97` | SPEC-63acef08 | 履歴保存 |
| `SPEC-8ae67d67` | - | 🗑️ 廃止 |

### 基盤SPEC（依存なし・他が依存）

以下のSPECは依存がなく、他のSPECから依存される基盤機能です：

- **SPEC-66555000**: ロードバランサー主導エンドポイント登録（最上位基盤、SPEC-94621a1fを置換）
- **SPEC-dcaeaec4**: モデルストレージ
- **SPEC-d7feaa2c**: エンジンローダー
- **SPEC-799b8e2b**: 共通ログシステム
- **SPEC-4b6e9f2a**: クラウドプレフィックスルーティング

## 🔧 コアシステム

| SPEC ID | 機能名 | Status | Priority | 実装 |
|---------|--------|--------|----------|------|
| `SPEC-05098000` | 推論中ノードへの多重リクエストキューイング | ✅ | P1 | ✅ |
| `SPEC-24157000` | OpenAI互換API完全準拠 | ✅ | P1 | ✅ |
| `SPEC-32e2b31a` | LLM Load Balancer System（統合版・アーカイブ） | ✅ | P1 | ✅ |
| `SPEC-35375000` | ロードバランサー負荷最適化 | ✅ | P1 | ✅ |
| `SPEC-3fc2c1e4` | SPEC-3fc2c1e4: 実行エンジン（統合仕様） | ✅ | P2 | ✅ |
| `SPEC-443acc8c` | 廃止: ヘルスチェックシステム（→SPEC-66555000） | 🗑️ | - | - |
| `SPEC-589f2df1` | ロードバランシングシステム | ✅ | P1 | ✅ |
| `SPEC-5cd7b614` | GPU必須ノード登録要件 | ✅ | P1 | ✅ |
| `SPEC-63acef08` | 統一APIプロキシ | ✅ | P1 | ✅ |
| `SPEC-66555000` | ロードバランサー主導エンドポイント登録システム | ✅ | P1 | ✅ |
| `SPEC-94621a1f` | 廃止: ノード自己登録システム（→SPEC-66555000） | 🗑️ | - | - |
| `SPEC-d7feaa2c` | SPEC-d7feaa2c: Nodeエンジンローダー抽象化とNemotron直接ロード | ✅ | P1 | ✅ |

## 🔐 認証・セキュリティ

| SPEC ID | 機能名 | Status | Priority | 実装 |
|---------|--------|--------|----------|------|
| `SPEC-7c0a37e0` | APIキースコープシステム | ✅ | P1 | ✅ |
| `SPEC-d4eb8796` | ロードバランサー認証・アクセス制御 | ✅ | P1 | ✅ |

## 📦 モデル管理

| SPEC ID | 機能名 | Status | Priority | 実装 |
|---------|--------|--------|----------|------|
| `SPEC-08d2b908` | SPEC-08d2b908: モデル管理（統合仕様） | ✅ | P2 | ✅ |
| `SPEC-0c4f3e5c` | 廃止: LLM runtimeモデルストレージ形式サポート | 🗑️ | - | - |
| `SPEC-11106000` | Hugging Face URL 登録（変換なし） | ✅ | P1 | ✅ |
| `SPEC-3df1b977` | 廃止: モデルファイル破損時の自動修復機能 | 🗑️ | - | - |
| `SPEC-47649000` | モデルメタデータSQLite統合 | ✅ | P1 | ✅ |
| `SPEC-48678000` | モデル自動解決機能 | ✅ | P2 | ✅ |
| `SPEC-6c2d9f1e` | モデル登録キャッシュとマルチモーダルI/Oの完全動作 | ✅ | P1 | ✅ |
| `SPEC-6cd7f960` | 対応モデルリスト型管理 | ✅ | P1 | ✅ |
| `SPEC-69549000` | safetensors.cpp - safetensors直接推論ライブラリ | ✅ | P1 | ✅ |
| `SPEC-93536000` | ノードベースモデル管理とモデル対応ルーティング | ✅ | P1 | ✅ |
| `SPEC-82491000` | クラウドプロバイダーモデル一覧統合 | ✅ | P2 | ✅ |
| `SPEC-8a2d1d43` | gptossアーキテクチャエイリアスサポート | ✅ | P2 | ✅ |
| `SPEC-2c0e5a9b` | SPEC-2c0e5a9b: gpt-oss-20b safetensors 実行（GPU: Metal/DirectML） | ✅ | P2 | ✅ |
| `SPEC-a61b24f2` | 廃止: モデル形式選択（safetensors/GGUF）とGGUF選択ポリシー | 🗑️ | - | - |
| `SPEC-8ae67d67` | 廃止: ロードバランサー主導のモデル自動配布機能 | 🗑️ | - | - |
| `SPEC-dcaeaec4` | SPEC-dcaeaec4: LLM-Load Balancer独自モデルストレージ | ✅ | P1 | ✅ |
| `SPEC-e03a404c` | 画像認識モデル対応（Image Understanding） | ✅ | P2 | ✅ |

## 🛤️ ルーティング

| SPEC ID | 機能名 | Status | Priority | 実装 |
|---------|--------|--------|----------|------|
| `SPEC-32637000` | モデル capabilities に基づくルーティング検証 | ✅ | P1 | ✅ |
| `SPEC-4b6e9f2a` | クラウドモデルプレフィックスルーティング | ✅ | P2 | ✅ |

## 🎨 マルチモーダル対応

| SPEC ID | 機能名 | Status | Priority | 実装 |
|---------|--------|--------|----------|------|
| `SPEC-26006000` | 音声モデル対応（TTS + ASR） | ✅ | P1 | ✅ |
| `SPEC-5fc9fe92` | Playground Chat マルチモーダル対応 | ✅ | P2 | ✅ |
| `SPEC-ae3f974e` | 画像生成モデル対応（Image Generation） | ✅ | P1 | ✅ |

## 🖥️ UI・ダッシュボード

| SPEC ID | 機能名 | Status | Priority | 実装 |
|---------|--------|--------|----------|------|
| `SPEC-026b2cde` | リクエスト履歴一覧のページネーション機能 | ✅ | P2 | ✅ |
| `SPEC-712c20cf` | 管理ダッシュボード | ✅ | P2 | ✅ |
| `SPEC-a7e6d40a` | CLI インターフェース整備 | ✅ | P2 | ✅ |

## 📊 ログ・履歴

| SPEC ID | 機能名 | Status | Priority | 実装 |
|---------|--------|--------|----------|------|
| `SPEC-1970e39f` | 構造化ロギング強化 | ✅ | P2 | ✅ |
| `SPEC-1f2a9c3d` | SPEC-log-api: Node / Load Balancer Log Retrieval API | ✅ | P2 | ✅ |
| `SPEC-799b8e2b` | 共通ログシステム | ✅ | P1 | ✅ |
| `SPEC-fbc50d97` | リクエスト/レスポンス履歴保存機能 | ✅ | P1 | ✅ |

## 🚀 CI/CD・自動化

| SPEC ID | 機能名 | Status | Priority | 実装 |
|---------|--------|--------|----------|------|
| `SPEC-47c6f44c` | 自動マージ機能の実装 | ✅ | P2 | ✅ |
| `SPEC-dc648675` | Worktree環境での作業境界強制システム | ✅ | P2 | ✅ |
| `SPEC-ee2aa3ef` | 完全自動化リリースシステム | ✅ | P2 | ✅ |

## 📁 その他

| SPEC ID | 機能名 | Status | Priority | 実装 |
|---------|--------|--------|----------|------|
| `SPEC-83825900` | Nemotron CUDA PoC | ✅ | P3 | ✅ |
| `SPEC-ea015fbb` | Web UI 画面一覧 | ✅ | P2 | ✅ |
| `SPEC-efff1da7` | SPEC-efff1da7: Nemotron safetensors-cpp PoC | ✅ | P3 | ✅ |

## 優先度サマリー

### P1（最優先）- 22件

| SPEC ID | 機能名 | 状態 |
|---------|--------|------|
| `SPEC-05098000` | 推論中ノードへの多重リクエストキューイング | 完了 |
| `SPEC-24157000` | OpenAI互換API完全準拠 | 完了 |
| `SPEC-11106000` | Hugging Face URL 登録（変換なし） | 完了 |
| `SPEC-26006000` | 音声モデル対応（TTS + ASR） | 完了 |
| `SPEC-32637000` | モデル capabilities に基づくルーティング検証 | 完了 |
| `SPEC-32e2b31a` | LLM Load Balancer System（統合版・アーカイブ） | 完了 |
| `SPEC-35375000` | ロードバランサー負荷最適化 | 完了 |
| `SPEC-47649000` | モデルメタデータSQLite統合 | 完了 |
| `SPEC-589f2df1` | ロードバランシングシステム | 完了 |
| `SPEC-5cd7b614` | GPU必須ノード登録要件 | 完了 |
| `SPEC-63acef08` | 統一APIプロキシ | 完了 |
| `SPEC-66555000` | ロードバランサー主導エンドポイント登録システム | 完了 |
| `SPEC-6c2d9f1e` | モデル登録キャッシュとマルチモーダルI/Oの完全動作 | 完了 |
| `SPEC-6cd7f960` | 対応モデルリスト型管理 | 完了 |
| `SPEC-69549000` | safetensors.cpp - safetensors直接推論ライブラリ | 完了 |
| `SPEC-93536000` | ノードベースモデル管理とモデル対応ルーティング | 完了 |
| `SPEC-799b8e2b` | 共通ログシステム | 完了 |
| `SPEC-7c0a37e0` | APIキースコープシステム | 完了 |
| `SPEC-ae3f974e` | 画像生成モデル対応（Image Generation） | 完了 |
| `SPEC-d4eb8796` | ロードバランサー認証・アクセス制御 | 完了 |
| `SPEC-d7feaa2c` | SPEC-d7feaa2c: Nodeエンジンローダー抽象化とNemotron直接ロード | 完了 |
| `SPEC-dcaeaec4` | SPEC-dcaeaec4: LLM-Load Balancer独自モデルストレージ | 完了 |
| `SPEC-fbc50d97` | リクエスト/レスポンス履歴保存機能 | 完了 |

### P2（重要）- 18件

| SPEC ID | 機能名 | 状態 |
|---------|--------|------|
| `SPEC-026b2cde` | リクエスト履歴一覧のページネーション機能 | 完了 |
| `SPEC-08d2b908` | SPEC-08d2b908: モデル管理（統合仕様） | 完了 |
| `SPEC-1970e39f` | 構造化ロギング強化 | 完了 |
| `SPEC-1f2a9c3d` | SPEC-log-api: Node / Load Balancer Log Retrieval API | 完了 |
| `SPEC-2c0e5a9b` | SPEC-2c0e5a9b: gpt-oss-20b safetensors 実行（GPU: Metal/DirectML） | 完了 |
| `SPEC-3fc2c1e4` | SPEC-3fc2c1e4: 実行エンジン（統合仕様） | 完了 |
| `SPEC-47c6f44c` | 自動マージ機能の実装 | 完了 |
| `SPEC-48678000` | モデル自動解決機能 | 完了 |
| `SPEC-4b6e9f2a` | クラウドモデルプレフィックスルーティング | 完了 |
| `SPEC-5fc9fe92` | Playground Chat マルチモーダル対応 | 完了 |
| `SPEC-712c20cf` | 管理ダッシュボード | 完了 |
| `SPEC-82491000` | クラウドプロバイダーモデル一覧統合 | 完了 |
| `SPEC-8a2d1d43` | gptossアーキテクチャエイリアスサポート | 完了 |
| `SPEC-a7e6d40a` | CLI インターフェース整備 | 完了 |
| `SPEC-dc648675` | Worktree環境での作業境界強制システム | 完了 |
| `SPEC-e03a404c` | 画像認識モデル対応（Image Understanding） | 完了 |
| `SPEC-ea015fbb` | Web UI 画面一覧 | 完了 |
| `SPEC-ee2aa3ef` | 完全自動化リリースシステム | 完了 |

### P3（通常）- 2件

| SPEC ID | 機能名 | 状態 |
|---------|--------|------|
| `SPEC-83825900` | Nemotron CUDA PoC | 完了 |
| `SPEC-efff1da7` | SPEC-efff1da7: Nemotron safetensors-cpp PoC | 完了 |

### 廃止 - 6件

| SPEC ID | 機能名 | 置換先 |
|---------|--------|--------|
| `SPEC-0c4f3e5c` | LLM runtimeモデルストレージ形式サポート | SPEC-dcaeaec4 |
| `SPEC-3df1b977` | モデルファイル破損時の自動修復機能 | SPEC-48678000 |
| `SPEC-443acc8c` | ヘルスチェックシステム | SPEC-66555000 |
| `SPEC-8ae67d67` | ロードバランサー主導のモデル自動配布機能 | SPEC-dcaeaec4 |
| `SPEC-94621a1f` | ノード自己登録システム | SPEC-66555000 |
| `SPEC-a61b24f2` | モデル形式選択（safetensors/GGUF） | 統合仕様へ移行 |
