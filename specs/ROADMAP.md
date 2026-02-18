# LLM Load Balancer ロードマップ

> 更新: 2026-02-04 (specs/specs.md の整合を反映)
>
> 本ドキュメントはSPEC整理に基づき作成されました。

## 実装状況サマリー

| ステータス | 件数 | 割合 |
|-----------|------|------|
| ✅ 実装済み | 49 | 86% |
| 🚧 実装中 | 1 | 2% |
| 🗑️ 廃止 | 7 | 12% |
| **合計** | **57** | 100% |

## 実装状況マトリクス

### コアシステム (15件)

| SPEC ID | 機能名 | ステータス | 優先度 |
|---------|--------|-----------|--------|
| SPEC-da253baa | 推論中ノードへの多重リクエストキューイング | ✅ 済 | P1 |
| SPEC-0f1de549 | OpenAI互換API完全準拠 | ✅ 済 | P1 |
| SPEC-92a1bd54 | Open Responses API対応 | ✅ 済 | P1 |
| SPEC-32e2b31a | LLM Load Balancer System（統合版・アーカイブ） | ✅ 済 | P1 |
| SPEC-0305fd6c | ロードバランサー負荷最適化 | ✅ 済 | P1 |
| SPEC-f8e3a1b7 | llmlb アーキテクチャ刷新 | ✅ 済 | P1 |
| SPEC-3fc2c1e4 | 実行エンジン（統合仕様） | ✅ 済 | P2 |
| SPEC-443acc8c | 廃止: ヘルスチェックシステム（→SPEC-e8e9326e） | 🗑️ 廃止 | - |
| SPEC-589f2df1 | ロードバランシングシステム | ✅ 済 | P1 |
| SPEC-5cd7b614 | GPU必須ノード登録要件 | ✅ 済 | P1 |
| SPEC-63acef08 | 統一APIプロキシ | ✅ 済 | P1 |
| SPEC-e8e9326e | ロードバランサー主導エンドポイント登録システム | ✅ 済 | P1 |
| SPEC-94621a1f | 廃止: ノード自己登録システム（→SPEC-e8e9326e） | 🗑️ 廃止 | - |
| SPEC-2f441f93 | 廃止: ノード登録承認フロー（NodeRegistry廃止） | 🗑️ 廃止 | - |
| SPEC-d7feaa2c | Nodeエンジンローダー抽象化とNemotron直接ロード (moved to xLLM repo) | ✅ 済 | P1 |

### 認証・セキュリティ (2件)

| SPEC ID | 機能名 | ステータス | 優先度 |
|---------|--------|-----------|--------|
| SPEC-7c0a37e0 | APIキースコープシステム | ✅ 済 | P1 |
| SPEC-d4eb8796 | ロードバランサー認証・アクセス制御 | ✅ 済 | P1 |

### モデル管理 (18件)

| SPEC ID | 機能名 | ステータス | 優先度 |
|---------|--------|-----------|--------|
| SPEC-08d2b908 | モデル管理（統合仕様） | ✅ 済 | P2 |
| SPEC-0c4f3e5c | 廃止: LLM runtimeモデルストレージ形式サポート | 🗑️ 廃止 | - |
| SPEC-68551ec8 | Hugging Face URL 登録（変換なし） | ✅ 済 | P1 |
| SPEC-3df1b977 | 廃止: モデルファイル破損時の自動修復機能 | 🗑️ 廃止 | - |
| SPEC-5f3dd53a | Windows CUDA runtime DLL (gpt-oss/nemotron) | 🚧 部分 | P2 |
| SPEC-f408aae2 | モデルメタデータSQLite統合 | ✅ 済 | P1 |
| SPEC-48678000 | モデル自動解決機能 (moved to xLLM repo) | ✅ 済 | P2 |
| SPEC-6c2d9f1e | モデル登録キャッシュとマルチモーダルI/Oの完全動作 | ✅ 済 | P1 |
| SPEC-6cd7f960 | 対応モデルリスト型管理 | ✅ 済 | P1 |
| SPEC-69549000 | safetensors.cpp (moved to xLLM repo) - safetensors直接推論ライブラリ | ✅ 済 | P1 |
| SPEC-93536000 | ノードベースモデル管理とモデル対応ルーティング (moved to xLLM repo) | ✅ 済 | P1 |
| SPEC-996e37bf | クラウドプロバイダーモデル一覧統合 | ✅ 済 | P2 |
| SPEC-8a2d1d43 | gptossアーキテクチャエイリアスサポート | ✅ 済 | P2 |
| SPEC-2c0e5a9b | gpt-oss-20b safetensors 実行（GPU: Metal/DirectML） | ✅ 済 | P2 |
| SPEC-a61b24f2 | 廃止: モデル形式選択（safetensors/GGUF）とGGUF選択ポリシー | 🗑️ 廃止 | - |
| SPEC-8ae67d67 | 廃止: ロードバランサー主導のモデル自動配布機能 | 🗑️ 廃止 | - |
| SPEC-dcaeaec4 | LLM-Load Balancer独自モデルストレージ | ✅ 済 | P1 |
| SPEC-e03a404c | 画像認識モデル対応（Image Understanding） | ✅ 済 | P2 |

### ルーティング (2件)

| SPEC ID | 機能名 | ステータス | 優先度 |
|---------|--------|-----------|--------|
| SPEC-dcf8677f | モデル capabilities に基づくルーティング検証 | ✅ 済 | P1 |
| SPEC-4b6e9f2a | クラウドモデルプレフィックスルーティング | ✅ 済 | P2 |

### マルチモーダル対応 (3件)

| SPEC ID | 機能名 | ステータス | 優先度 |
|---------|--------|-----------|--------|
| SPEC-617247d2 | 音声モデル対応（TTS + ASR） | ✅ 済 | P1 |
| SPEC-5fc9fe92 | Playground Chat マルチモーダル対応 | ✅ 済 | P2 |
| SPEC-ae3f974e | 画像生成モデル対応（Image Generation） | ✅ 済 | P1 |

### UI・ダッシュボード (2件)

| SPEC ID | 機能名 | ステータス | 優先度 |
|---------|--------|-----------|--------|
| SPEC-026b2cde | リクエスト履歴一覧のページネーション機能 | ✅ 済 | P2 |
| SPEC-712c20cf | 管理ダッシュボード | ✅ 済 | P2 |

### CLI・運用 (3件)

| SPEC ID | 機能名 | ステータス | 優先度 |
|---------|--------|-----------|--------|
| SPEC-787a0b27 | llmlb serveコマンドのシングル実行制約 | ✅ 済 | P1 |
| SPEC-669176b2 | llmlb CLIコマンド | ✅ 済 | P1 |
| SPEC-a7e6d40a | CLI インターフェース整備 (moved to xLLM repo) | ✅ 済 | P2 |

### ログ・履歴 (5件)

| SPEC ID | 機能名 | ステータス | 優先度 |
|---------|--------|-----------|--------|
| SPEC-1970e39f | 構造化ロギング強化 | ✅ 済 | P2 |
| SPEC-1f2a9c3d | Node / Load Balancer Log Retrieval API | ✅ 済 | P2 |
| SPEC-5045f436 | トークン累積統計機能 | ✅ 済 | P1 |
| SPEC-799b8e2b | 共通ログシステム | ✅ 済 | P1 |
| SPEC-fbc50d97 | リクエスト/レスポンス履歴保存 | ✅ 済 | P1 |

### CI/CD・自動化 (4件)

| SPEC ID | 機能名 | ステータス | 優先度 |
|---------|--------|-----------|--------|
| SPEC-5da87697 | CI/CD パイプライン | ✅ 済 | P2 |
| SPEC-47c6f44c | 自動マージ機能の実装 | ✅ 済 | P2 |
| SPEC-dc648675 | Worktree環境での作業境界強制システム | ✅ 済 | P2 |
| SPEC-ee2aa3ef | 完全自動化リリースシステム | ✅ 済 | P2 |

### その他 (3件)

| SPEC ID | 機能名 | ステータス | 優先度 |
|---------|--------|-----------|--------|
| SPEC-55ebd062 | Nemotron CUDA PoC | ✅ 済 | P3 |
| SPEC-ea015fbb | Web UI 画面一覧 | ✅ 済 | P2 |
| SPEC-efff1da7 | Nemotron safetensors-cpp PoC | ✅ 済 | P3 |

## 新規SPEC（予定）

| SPEC ID | 機能名 | 説明 | 優先度 |
|---------|--------|------|--------|
| TBD | Nemotronテキスト生成 | 現在検証のみ→推論実装 | P2 |

## マイルストーン

### Milestone 1: 認証・セキュリティ強化

**目標**: APIキースコープと認証基盤の完成

| 優先度 | SPEC | 依存関係 |
|--------|------|---------|
| P1 | SPEC-d4eb8796 ロードバランサー認証 | なし |
| P1 | SPEC-7c0a37e0 APIキースコープ | SPEC-d4eb8796 |

### Milestone 2: マルチモーダル対応

**目標**: 音声・画像のOpenAI互換API完成

| 優先度 | SPEC | 依存関係 |
|--------|------|---------|
| P1 | SPEC-617247d2 音声モデル（TTS+ASR） | Milestone 1 |
| P1 | SPEC-ae3f974e 画像生成 | Milestone 1 |
| P1 | SPEC-e03a404c 画像認識 | Milestone 1 |
| P2 | SPEC-dcf8677f capabilities検証 | マルチモーダルSPEC |

### Milestone 3: モデル管理強化

**目標**: モデル管理の安定性・ユーザビリティ向上

| 優先度 | SPEC | 依存関係 |
|--------|------|---------|
| P2 | SPEC-f408aae2 SQLite統合 | なし |
| P2 | SPEC-48678000 モデル自動解決 (moved to xLLM repo) | SPEC-f408aae2 |
| P2 | SPEC-68551ec8 HF URL登録 | SPEC-f408aae2 |
| P2 | SPEC-0c4f3e5c ストレージ形式 | SPEC-f408aae2 |

### Milestone 4: パフォーマンス・品質

**目標**: 高負荷対応とログ基盤強化

| 優先度 | SPEC | 依存関係 |
|--------|------|---------|
| P2 | SPEC-0305fd6c 負荷最適化 | Milestone 1 |
| P3 | SPEC-1970e39f 構造化ロギング | なし |
| P3 | SPEC-799b8e2b 共通ログシステム | SPEC-1970e39f |

## 依存関係図

```text
Milestone 1 (認証)
    ├── SPEC-d4eb8796 (認証基盤)
    │   └── SPEC-7c0a37e0 (APIキースコープ)
    │
    ▼
Milestone 2 (マルチモーダル)
    ├── SPEC-617247d2 (音声)
    ├── SPEC-ae3f974e (画像生成)
    ├── SPEC-e03a404c (画像認識)
    └── SPEC-dcf8677f (capabilities検証)
    │
    ▼
Milestone 3 (モデル管理)
    ├── SPEC-f408aae2 (SQLite統合)
    │   ├── SPEC-48678000 (自動解決, moved to xLLM repo)
    │   ├── SPEC-68551ec8 (HF登録)
    │   └── SPEC-0c4f3e5c (ストレージ形式)
    │
    ▼
Milestone 4 (品質)
    ├── SPEC-0305fd6c (負荷最適化)
    └── SPEC-1970e39f → SPEC-799b8e2b (ログ)
```

## 廃止されたSPEC

| SPEC ID | 機能名 | 廃止理由 | 置換先 |
|---------|--------|---------|--------|
| SPEC-0c4f3e5c | LLM runtimeモデルストレージ形式 | 統合設計へ移行 | SPEC-dcaeaec4 |
| SPEC-3df1b977 | モデルファイル破損自動修復 | シンプル設計へ移行 | SPEC-48678000 (moved to xLLM repo) |
| SPEC-443acc8c | ヘルスチェックシステム | エンドポイント登録へ統合 | SPEC-e8e9326e |
| SPEC-8ae67d67 | ロードバランサー主導のモデル自動配布 | ノード主導pull方式へ変更 | SPEC-dcaeaec4 |
| SPEC-94621a1f | ノード自己登録システム | エンドポイント登録へ移行 | SPEC-e8e9326e |
| SPEC-2f441f93 | ノード登録承認フロー | NodeRegistry廃止 | SPEC-e8e9326e |
| SPEC-a61b24f2 | モデル形式選択（safetensors/GGUF） | Node側形式選択へ統一 | 統合仕様 |

## 更新履歴

- 2026-01-02: 廃止SPEC整理（4件）、実装状況をspecs.mdと整合
- 2025-12-24: 初版作成（33 SPEC整理）
