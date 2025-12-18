# 実装計画: Playground Chat マルチモーダル対応

**機能ID**: `SPEC-5fc9fe92` | **日付**: 2025-12-13 | **仕様**: [spec.md](./spec.md)  
**入力**: `/specs/SPEC-5fc9fe92/spec.md` の機能仕様

## 概要

Playground Chat を **テキスト・画像・音声** の入力に対応させます。
現状のPlaygroundはテキストのみを前提としており、UIも `/v1/chat/completions` に送信する
`messages[].content` も文字列扱いです。これを拡張し、画像/音声を添付できるUIと、
OpenAI互換のマルチモーダル入力形式（content parts）を扱えるようにします。

さらに、画像/音声は「ファイル添付」だけでなく **貼り付け（ペースト）** でも入力できるようにし、
生成AIの回答に画像/音声が含まれる場合は **会話ログ上で表示（プレビュー/再生）** できるようにします。

本計画は「まず価値の大きい画像（P1）を成立させ、その後に音声（P2）を追加する」順で進めます。
テキストのみの既存体験（送信、ストリーミング、停止）を壊さないことを最優先とします（P3）。

## 技術コンテキスト

**言語/バージョン**:
- フロントエンド: TypeScript / React（Dashboard/Playground）
- ルーター: Rust（edition 2021）
- ノード: C++（C++20）

**主要依存関係**:
- フロント: Vite + React, TanStack Query, shadcn/ui（既存）
- ルーター: axum, reqwest, serde_json（既存）
- ノード: nlohmann/json, llama.cpp 統合（既存）

**ストレージ**:
- Playground のセッションは localStorage（既存）
- ルーターはリクエスト履歴を保持（既存）

**テスト**:
- Rust: `cargo test`
- E2E: Playwright（UI存在確認が中心）

**対象プラットフォーム**: ローカル開発環境（macOS/Linux想定）＋ブラウザ

## 憲章チェック

**シンプルさ**:
- 新しい永続ストレージ（DB等）を追加しない
- 既存の OpenAI 互換エンドポイント（`/v1/chat/completions`）を拡張して扱う
- 添付データは「送信時の入力」として扱い、UIのセッション永続化には原則含めない（容量・プライバシーの理由）

**テスト**:
- 仕様の受け入れ条件が満たせる最低限の自動テスト（contract + UI）を追加する
- 少なくとも「添付UIが出る」「添付が送信payloadに反映される」ことを検証可能にする

**可観測性**:
- ルーターのリクエスト履歴が肥大化しないよう、添付データは保存時に要約/伏せ字化する（詳細はresearch.md）

## プロジェクト構造

### ドキュメント（この機能）

```
specs/SPEC-5fc9fe92/
├── spec.md              # 機能仕様書
├── plan.md              # このファイル
├── research.md          # Phase 0 出力
├── data-model.md        # Phase 1 出力
├── quickstart.md        # Phase 1 出力
├── contracts/           # Phase 1 出力
└── tasks.md             # Phase 2 出力（/speckit.tasksで作成）
```

### 変更が入る可能性が高いコード領域（目安）

```
router/src/web/dashboard/src/pages/Playground.tsx
router/src/web/dashboard/src/lib/api.ts
router/src/api/openai.rs
node/src/api/openai_endpoints.cpp
```

## Phase 0: アウトライン＆リサーチ（research.md）

**目的**: マルチモーダル入力形式・制約・互換性・ログ方針を確定する。  
**出力**: [research.md](./research.md)

本件は「OpenAI互換」を前提に、Playgroundの入力を **content parts** で表現する方針とします。
併せて、添付のサイズ制約、保存方針（セッション/履歴）を決定します。

## Phase 1: 設計＆契約（data-model / contracts / quickstart）

**目的**: UIとAPIの境界（何を送るか/何を表示するか）を明確化し、テスト可能な契約を用意する。  
**出力**:
- [data-model.md](./data-model.md)
- [contracts/playground-multimodal-api.json](./contracts/playground-multimodal-api.json)
- [quickstart.md](./quickstart.md)

### 設計ポイント

1. **UIの入力モデル**
   - テキスト入力 + 添付/貼り付け（画像/音声）の「下書き」状態を持つ
   - 送信前にプレビューと削除ができる

2. **APIへの送信モデル（OpenAI互換）**
   - 添付がない場合: 既存どおり `content: string`
   - 添付がある場合: `content: [{type:"text",...}, {type:"image_url" or "input_audio", ...}]` のような「parts」表現を用いる

3. **生成結果（画像/音声）の表示**
   - 回答本文に含まれる画像/音声を会話ログ上で表示（プレビュー/再生）できる
   - URLやデータURLなど、ユーザーが「結果を確認」できる表現を優先する

4. **モデル対応状況の提示**
   - 選択中モデルが画像/音声に対応していない場合は添付操作を無効化する
   - 「対応状況の根拠」をUIで示す（例: “未対応/不明/対応”）

5. **履歴/監査ログの取り扱い**
   - ルーターのリクエスト履歴に添付の生データ（base64等）を保存しない
   - 保存時は要約（種別、サイズ、ハッシュ等）に置換する

## Phase 2: タスク計画アプローチ（tasks.mdはここでは作らない）

`spec.md` のユーザーストーリー（P1/P2/P3）を縦に切り、次の順でタスク化します。

1. P1（画像）: UI添付 → payload生成 → 表示 → エラー → 履歴の肥大化回避  
2. P2（音声）: UI添付 → payload生成 → 表示（再生） → エラー  
3. P2（生成物表示）: 回答内の画像/音声を表示（プレビュー/再生） → 失敗時のフォールバック  
4. P3（互換性）: テキストのみ/ストリーミング/停止の回帰防止  

並列化は「異なるファイル・依存関係なし」を条件に `[P]` を付けます。

## 進捗トラッキング

- [x] Phase 0: Research完了（`research.md`）
- [x] Phase 1: Design完了（`data-model.md`, `contracts/`, `quickstart.md`）
- [x] Phase 2: Tasks生成（`tasks.md`）
