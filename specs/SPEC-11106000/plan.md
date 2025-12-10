# 実装計画: Hugging Face GGUFモデル対応登録

**機能ID**: `SPEC-11106000` | **日付**: 2025-12-01 | **仕様**: specs/SPEC-11106000/spec.md  
**入力**: `/specs/SPEC-11106000/spec.md` の機能仕様

## 概要
- HFカタログUIは廃止し、単一テキストエリアにHFリポジトリ/ファイルURLを貼るだけで登録。
- GGUFがあれば即登録・ダウンロード、なければ自動で非GGUF→GGUF変換タスクをキュー。
- /v1/models には実体GGUFが存在するものだけ返す。重複はエラー。
- 失敗した変換/ダウンロードはUIの「Restore」で再キューできる。

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
- HF API: repoメタ（siblings）取得と認証要否のみ確認。カタログ一覧は扱わない。
- モデルID命名: `hf/{repo}/{filename}` 固定。
- 非GGUF変換: ルーター側で一度だけ `convert_hf_to_gguf.py` を使う。ダミー変換フラグは使わず、実変換を前提にする。

## Phase 1: 設計＆契約
- data-model.md: ModelInfo 拡張（source, download_url, status, size, repo/filename）。
- contracts:  
  - `POST /api/models/register` repo-only/filename指定、GGUF/非GGUF、自動変換。  
  - `POST /api/models/convert` 失敗時の再キュー。  
  - `GET /api/models/convert` タスク一覧。  
  - `/v1/models` は実体があるもののみ。  
- quickstart.md: URL貼付→登録→（変換）→/v1/models までの手順、Restore手順を記載。

## Phase 2: タスク計画アプローチ
- Contract tests: register（repo/file, non-GGUF→convert, duplicate, not-found）、/v1/models 実体のみ、convert RESTORE再キュー。
- Integration: HF siblingsモック→ファイル選択→convertキュー→完了後 /v1/models 出現。
- Frontend: URL登録フォーム、バナー、登録済みリスト、失敗タスクのRestoreボタンのE2E。
- CLI: scope外（今回はWeb/API中心）※必要なら後追い。
- 実装: registry永続化、convert manager、/v1/models フィルタ、RestoreボタンのAPI連携。

## 複雑さトラッキング
| 違反 | 必要な理由 | 代替案を却下した理由 |
|------|-----------|----------------------|
| 非GGUF→GGUF変換をルーター側で実行 | /v1/models を実体ありに限定するため | ノード側変換は台数分の負荷・再現性低下になるため |

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
