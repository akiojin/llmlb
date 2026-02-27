# 実装計画: Hugging Face URL 登録（変換なし・メタデータ登録）

**機能ID**: `SPEC-68551ec8` | **日付**: 2025-12-01 | **仕様**: specs/SPEC-68551ec8/spec.md  
**入力**: `/specs/SPEC-68551ec8/spec.md` の機能仕様

## 概要
- HFリポジトリ/ファイルURLを登録し、ロードバランサーは**メタデータとマニフェストのみ**を保持する。
- **モデルバイナリはロードバランサーに保持せず**、NodeがHFから直接ダウンロードしてキャッシュする。
- /v1/models は登録済みモデルを返し、ready はNodeの同期状態に従う。

## 技術コンテキスト
- **言語/バージョン**: Rust 1.75+（llmlb/cli）、TypeScript/JSなしのプレーン JS (web static)、C++ノードは変更最小。
- **主要依存関係**: router: axum/reqwest/serde; web: vanilla JS + fetch; cli: existing router CLI基盤を再利用（要確認）。
- **ストレージ**: 既存DB/registryそのまま（モデル情報を拡張）。
- **テスト**: cargo test (router)、JSは軽量ユニット or 集約E2E（既存フレームに合わせる）。
- **対象プラットフォーム**: Linux (server)、ブラウザ（現行ダッシュボード）。
- **プロジェクトタイプ**: web（backend + frontend + cli）。
- **パフォーマンス目標**: HFメタ取得 P95 3s以内、登録反映 5s以内、同期ポーリング5s間隔。
- **制約**: HF API レートリミット; Nodeはmanifestに従い外部ソースから直接取得。
- **スケール/スコープ**: 対応モデル数 O(10〜100)、ノード O(10) 想定。

## 憲章チェック
**シンプルさ**: プロジェクト数=2(backend+frontend)＋既存cli; ラッパー追加なし; DTO最小。  
**アーキテクチャ**: 既存ライブラリ構成を踏襲。CLIは既存コマンドにサブコマンド追加。  
**テスト**: TDD順守。まず契約/統合テストを追加。  
**可観測性**: router ログ既存を活用、進捗は構造化ログ追加。  
**バージョニング**: SemVerリリースフロー前提。  
→ 初期憲章チェック: 合格（想定）

## プロジェクト構造
- docs: specs/SPEC-68551ec8/{research.md, data-model.md, quickstart.md, contracts/} を生成。  
- backend (router): src/api/models.rs, registry/models.rs 付近拡張。  
- frontend (web/static): models.js + UIテンプレート拡張。  
- cli: 既存 `llmlb` に `model list/add` サブコマンドを整理。

## Phase 0: アウトライン＆リサーチ
- HF API: repoメタ（siblings）取得と認証要否のみ確認。カタログ一覧は扱わない。
- モデルID命名: `hf/{repo}` または `hf/{repo}/{filename}` を基本形とする。
- 形式選択はロードバランサーで行わず、Nodeがruntime/GPU要件に応じて選択する。

## Phase 1: 設計＆契約
- data-model.md: ModelInfo 拡張（repo, filename?, source, status, size, artifacts）。
- contracts:
  - `POST /api/models/register` repo-only/filename指定（バイナリ取得なし）。
  - `GET /api/models/registry/:model_name/manifest.json` でNode向けマニフェストを提供。
  - `/v1/models` は登録済みモデルとready状態を返す。
- quickstart.md: URL貼付→登録→/v1/models 反映→Node同期の手順を記載。

## Phase 2: タスク計画アプローチ
- Contract tests: register（repo/file, duplicate, not-found）、/v1/models readyの表現。
- Integration: HF metadata取得→登録→manifest確認→/v1/models 反映。
- Frontend: URL登録フォームのみ、format/gguf_policy UIは削除。
- CLI: `model download` は削除またはNode側導線に変更。
- 実装: registry永続化、manifest生成、/v1/models ready整合。

## 複雑さトラッキング
| 違反 | 必要な理由 | 代替案を却下した理由 |
|------|-----------|----------------------|
| 該当なし | - | - |

## 進捗トラッキング
- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning更新（本チケットで反映）
- [x] Phase 3: Tasks更新反映
- [x] Phase 4: 実装完了
- [ ] Phase 5: 検証合格

**ゲート**
- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [ ] すべての要明確化解決済み
- [ ] 複雑さの逸脱を文書化済み
