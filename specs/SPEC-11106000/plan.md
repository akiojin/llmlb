# 実装計画: Hugging Face GGUFモデル対応登録

**機能ID**: `SPEC-11106000` | **日付**: 2025-12-01 | **仕様**: specs/SPEC-11106000/spec.md  
**入力**: `/specs/SPEC-11106000/spec.md` の機能仕様

## 概要
- HF の GGUF カタログを取得し、対応モデルと対応可能モデルを分離表示する。
- 選択した GGUF を対応モデルとして登録し、/v1/models・ダッシュボード・CLI で反映。
- 登録済みモデルに対し、全ノード／指定ノードへ「今すぐダウンロード」指示と進捗表示を提供。
- 将来の非GGUFはルーター側一括変換を想定（要方針決定）。

## 技術コンテキスト
- **言語/バージョン**: Rust 1.75+（router/cli）、TypeScript/JSなしのプレーン JS (web static)、C++ノードは変更最小。
- **主要依存関係**: router: axum/reqwest/serde; web: vanilla JS + fetch; cli: existing router CLI基盤を再利用（要確認）。
- **ストレージ**: 既存DB/registryそのまま（モデル情報を拡張）。
- **テスト**: cargo test (router)、JSは軽量ユニット or 集約E2E（既存フレームに合わせる）。
- **対象プラットフォーム**: Linux (server)、ブラウザ（現行ダッシュボード）。
- **プロジェクトタイプ**: web（backend + frontend + cli）。
- **パフォーマンス目標**: HF一覧 API 応答 P95 3s以内、登録反映 5s以内、進捗ポーリング5s間隔。
- **制約**: HF API レートリミット; ノードは manifest から自己ダウンロードする前提。
- **スケール/スコープ**: 対応モデル数 O(10〜100)、ノード O(10) 想定。

## 憲章チェック
**シンプルさ**: プロジェクト数=2(backend+frontend)＋既存cli; ラッパー追加なし; DTO最小。  
**アーキテクチャ**: 既存ライブラリ構成を踏襲。CLIは既存コマンドにサブコマンド追加。  
**テスト**: TDD順守。まず契約/統合テストを追加。  
**可観測性**: router ログ既存を活用、進捗は構造化ログ追加。  
**バージョニング**: semantic-release前提。  
→ 初期憲章チェック: 合格（想定）

## プロジェクト構造
- docs: specs/SPEC-11106000/{research.md, data-model.md, quickstart.md, contracts/} を生成。  
- backend (router): src/api/models.rs, registry/models.rs 付近拡張。  
- frontend (web/static): models.js + UIテンプレート拡張。  
- cli: 既存 `llm-router` に `model list/add/download` サブコマンド追加。

## Phase 0: アウトライン＆リサーチ
- HF API: GGUFタグ検索エンドポイント・レートリミット・認証要否。
- モデルID命名規則: repo+file から一意にする形式の決定。
- 非GGUF変換: どこで変換し、どこにホストするかの方針案をまとめる。

## Phase 1: 設計＆契約
- data-model.md: ModelInfo 拡張（source, download_url, status, size、hf_metaなど）。
- contracts/: OpenAPI 追記  
  - `GET /api/models/available` 拡張: source=hf/…、ページング・検索。  
  - `POST /api/models/register` (新) HF GGUFを対応モデルに追加。  
  - `POST /api/models/download` (新) target=all/specific node_ids。  
  - `GET /api/tasks/{id}` 既存進捗にモデルDL用途の例を追加。  
- quickstart.md: Web/CLI 両方の操作手順と進捗確認。

## Phase 2: タスク計画アプローチ
- tasks.md生成時に:  
  - Contract tests → 新APIの契約テスト (router)  
  - Integration → HFカタログ取得のモックテスト、ダウンロード指示→タスク生成→状態更新  
  - Frontend → UIレンダリング/状態遷移のE2E (軽量)  
  - CLI → コマンド挙動の統合テスト  
  - 実装 → registry保存、manifest生成/提供、進捗ポーリング

## 複雑さトラッキング
| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|----------------------------------|
| なし | - | - |

## 進捗トラッキング
- [x] Phase 0: Research完了 (/speckit.plan)
- [x] Phase 1: Design完了 (/speckit.plan)
- [ ] Phase 2: Task planning完了 (/speckit.plan - アプローチのみ)
- [ ] Phase 3: Tasks生成済み (/speckit.tasks)
- [ ] Phase 4: 実装完了
- [ ] Phase 5: 検証合格

**ゲート**
- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [ ] すべての要明確化解決済み
- [ ] 複雑さの逸脱を文書化済み
