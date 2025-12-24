# 機能仕様一覧

> 整理更新: 2025-12-24 02:35:00
>
> 総SPEC数: **41** | 廃止: 3 | plan.md欠損: 0

**凡例:** ✅ plan.md有り | 📋 plan.md無し | 🗑️ 廃止

## 🔧 コアシステム

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-32e2b31a` | LLM Router System（統合版・アーカイブ） | ✅ |
| `SPEC-35375000` | ルーター負荷最適化 | ✅ |
| `SPEC-443acc8c` | ヘルスチェックシステム | ✅ |
| `SPEC-589f2df1` | ロードバランシングシステム | ✅ |
| `SPEC-5cd7b614` | GPU必須ノード登録要件 | ✅ |
| `SPEC-63acef08` | 統一APIプロキシ | ✅ |
| `SPEC-94621a1f` | ノード自己登録システム | ✅ |

## 🔐 認証・セキュリティ

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-7c0a37e0` | APIキースコープシステム | ✅ |
| `SPEC-d4eb8796` | ルーター認証・アクセス制御 | ✅ |

## 📦 モデル管理

> **読み方**: 「統合仕様」は経緯を知らない人のための入口です。  
> まず統合仕様で**責務境界・原則・禁止事項**を把握し、詳細は各SPECへ進みます。

### 統合仕様

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-08d2b908` | モデル管理（統合仕様） | ✅ |

### 登録・配布・ストレージ

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-dcaeaec4` | LLM-Router独自モデルストレージ | ✅ |
| `SPEC-48678000` | モデル自動解決機能 | ✅ |
| `SPEC-47649000` | モデルメタデータSQLite統合 | ✅ |
| `SPEC-6c2d9f1e` | モデル登録キャッシュとマルチモーダルI/Oの完全動作 | ✅ |
| `SPEC-11106000` | Hugging Face URL 登録（形式選択・明示登録） | ✅ |

### 形式選択・アーティファクト

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-a61b24f2` | モデル形式選択（safetensors/GGUF）とGGUF選択ポリシー | ✅ |

### 廃止・置換済み

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-0c4f3e5c` | 廃止: LLM runtimeモデルストレージ形式サポート（置換: SPEC-dcaeaec4） | 🗑️ |
| `SPEC-3df1b977` | 廃止: モデルファイル破損時の自動修復機能（置換: SPEC-48678000） | 🗑️ |
| `SPEC-8ae67d67` | 廃止: ルーター主導のモデル自動配布機能 | 🗑️ |

## 🧠 実行エンジン・推論

> **読み方**: 「統合仕様」は実行エンジンの責務境界を示す入口です。  
> 詳細実装は個別SPEC（エンジン抽象化や特定モデル実行）を参照します。

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-3fc2c1e4` | 実行エンジン（統合仕様） | ✅ |
| `SPEC-d7feaa2c` | Nodeエンジンローダー抽象化とNemotron直接ロード | ✅ |
| `SPEC-2c0e5a9b` | gpt-oss-20b safetensors 実行（GPU: Metal/CUDA） | ✅ |
| `SPEC-8a2d1d43` | gptossアーキテクチャエイリアスサポート | ✅ |

## 🛤️ ルーティング

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-32637000` | モデル capabilities に基づくルーティング検証 | ✅ |
| `SPEC-4b6e9f2a` | クラウドモデルプレフィックスルーティング | ✅ |

## 🎨 マルチモーダル対応

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-26006000` | 音声モデル対応（TTS + ASR） | ✅ |
| `SPEC-5fc9fe92` | Playground Chat マルチモーダル対応 | ✅ |
| `SPEC-ae3f974e` | 画像生成モデル対応（Image Generation） | ✅ |
| `SPEC-e03a404c` | 画像認識モデル対応（Image Understanding） | ✅ |

## 🖥️ UI・ダッシュボード

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-026b2cde` | リクエスト履歴一覧のページネーション機能 | ✅ |
| `SPEC-712c20cf` | 管理ダッシュボード | ✅ |
| `SPEC-a7e6d40a` | CLI インターフェース整備 | ✅ |
| `SPEC-ea015fbb` | Web UI 画面一覧 | ✅ |

## 📊 ログ・履歴

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-1970e39f` | 構造化ロギング強化 | ✅ |
| `SPEC-1f2a9c3d` | SPEC-log-api: Node / Router Log Retrieval API | ✅ |
| `SPEC-799b8e2b` | 共通ログシステム | ✅ |
| `SPEC-fbc50d97` | リクエスト/レスポンス履歴保存機能 | ✅ |

## 🚀 CI/CD・自動化

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-47c6f44c` | 自動マージ機能の実装 | ✅ |
| `SPEC-dc648675` | Worktree環境での作業境界強制システム | ✅ |
| `SPEC-ee2aa3ef` | 完全自動化リリースシステム | ✅ |

## 🔬 PoC・調査

| SPEC ID | 機能名 | Status |
|---------|--------|--------|
| `SPEC-efff1da7` | Nemotron safetensors-cpp PoC | ✅ |

---

## 🔗 SPEC依存関係マトリクス

### 依存関係図

```text
                    SPEC-94621a1f (ノード自己登録)
                           │
                           ▼
                    SPEC-63acef08 (統一APIプロキシ)
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
        ▼                  ▼                  ▼
SPEC-443acc8c      SPEC-589f2df1      SPEC-4b6e9f2a
(ヘルスチェック)   (ロードバランシング) (クラウドルーティング)
        │
        └──────────────────┐
                           ▼
                    SPEC-d4eb8796 (認証・アクセス制御)
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
        ▼                  ▼                  ▼
SPEC-7c0a37e0      SPEC-712c20cf      SPEC-fbc50d97
(APIキースコープ)  (ダッシュボード)    (履歴保存)
                           │                  │
        ┌──────────────────┤                  │
        │                  │                  ▼
        ▼                  ▼          SPEC-026b2cde
SPEC-5fc9fe92      SPEC-ea015fbb      (ページネーション)
(マルチモーダルPG)  (画面一覧)
```

### 依存関係一覧

| SPEC ID | 機能名 | 依存先SPEC |
|---------|--------|-----------|
| `SPEC-63acef08` | 統一APIプロキシ | SPEC-94621a1f |
| `SPEC-443acc8c` | ヘルスチェック | SPEC-94621a1f |
| `SPEC-589f2df1` | ロードバランシング | SPEC-63acef08 |
| `SPEC-4b6e9f2a` | クラウドルーティング | SPEC-63acef08 |
| `SPEC-d4eb8796` | 認証・アクセス制御 | SPEC-443acc8c |
| `SPEC-7c0a37e0` | APIキースコープ | SPEC-d4eb8796 |
| `SPEC-712c20cf` | ダッシュボード | SPEC-94621a1f, SPEC-63acef08, SPEC-443acc8c, SPEC-d4eb8796 |
| `SPEC-fbc50d97` | 履歴保存 | SPEC-63acef08 |
| `SPEC-026b2cde` | ページネーション | SPEC-fbc50d97 |
| `SPEC-5fc9fe92` | Playgroundマルチモーダル | SPEC-712c20cf |
| `SPEC-ea015fbb` | 画面一覧 | SPEC-712c20cf, SPEC-d4eb8796 |
| `SPEC-08d2b908` | モデル管理（統合仕様） | SPEC-dcaeaec4, SPEC-11106000, SPEC-a61b24f2, SPEC-48678000, SPEC-6c2d9f1e |
| `SPEC-11106000` | Hugging Face URL 登録 | SPEC-a61b24f2, SPEC-dcaeaec4 |
| `SPEC-d7feaa2c` | Nodeエンジンローダー抽象化 | SPEC-08d2b908 |
| `SPEC-2c0e5a9b` | gpt-oss-20b safetensors 実行 | SPEC-d7feaa2c, SPEC-08d2b908 |
| `SPEC-3fc2c1e4` | 実行エンジン（統合仕様） | SPEC-08d2b908, SPEC-5cd7b614 |
| `SPEC-ee2aa3ef` | リリースシステム | SPEC-47c6f44c |

### 廃止・置換関係

| 廃止SPEC | 置換先SPEC |
|----------|-----------|
| `SPEC-0c4f3e5c` | SPEC-dcaeaec4 |
| `SPEC-3df1b977` | SPEC-48678000 |
| `SPEC-8ae67d67` | SPEC-dcaeaec4 |
