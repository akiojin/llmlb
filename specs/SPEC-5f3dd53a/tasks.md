# タスク: Windows CUDA runtime DLL (gpt-oss/nemotron)

**機能ID**: `SPEC-5f3dd53a`
**ステータス**: 未着手
**入力**: `specs/SPEC-5f3dd53a/spec.md`, `specs/SPEC-5f3dd53a/plan.md`

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能（異なるファイル、依存関係なし）
- 本仕様は xLLM (external repo) 側の実装が主となる

## Phase 0: Research

- [ ] T001 [P] CUDA DLL の提供元/ライセンスを確定（xLLM側で確認）
- [ ] T002 [P] 既存 gptoss_* C API 互換の CUDA 実装有無を確認
- [ ] T003 [P] CUDAアーティファクト命名規則の現状整理

## Phase 1: Design

- [ ] T004 DLL 探索順序（env -> model dir -> default）を確定
- [ ] T005 CUDAアーティファクト配置規則（model.cuda.bin / cuda/model.bin）を確定
- [ ] T006 エラー分類（DLL不足/アーティファクト不足/ロード失敗）を定義

## Phase 2: Implementation (xLLM repo)

- [ ] T007 `node/engines/gptoss/cuda/` に DLL ソースを追加
- [ ] T008 `node/engines/nemotron/cuda/` に DLL ソースを追加
- [ ] T009 CMake で `gptoss_cuda.dll` / `nemotron_cuda.dll` を生成
- [ ] T010 DLL 探索・ロードロジックを Node に実装
- [ ] T011 CUDAアーティファクト検出ロジックを Node に実装

## Phase 3: Tests

- [ ] T012 DLL 未配置時のエラー検証テストを追加
- [ ] T013 CUDAアーティファクト欠落時のエラー検証テストを追加
- [ ] T014 環境変数指定とモデルディレクトリ指定の両パスをテスト

## Phase 4: Docs

- [ ] T015 DLL 配置/環境変数/運用手順をドキュメント化
