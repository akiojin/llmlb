# 実装計画: Windows CUDA runtime DLL (gpt-oss/nemotron)

**機能ID**: `SPEC-5f3dd53a` | **日付**: 2026-01-06 | **仕様**: `specs/SPEC-5f3dd53a/spec.md`
**入力**: `specs/SPEC-5f3dd53a/spec.md`

## 実行フロー (/speckit.plan スコープ)
- Bash実行が拒否されたため、テンプレートに沿って手動で作成する。

## 概要
Windows CUDA主経路を成立させるため、gpt-oss / nemotron のCUDAランタイムDLLとCUDAアーティファクトの配置・解決方法を定義し、Nodeが明確なエラーで可否判定できるようにする。

## 技術コンテキスト
- 言語: C++ (Node)
- 対象OS/GPU: Windows + NVIDIA CUDA
- 実行形態: DLLロード (外部配布 or 同梱)
- 既存ABI: gptoss_* C API
- DirectML: 凍結 (本仕様外)

## 憲章チェック
- シンプルさ: DLL探索順序を固定し、フォールバックや自動変換はしない。
- 責務分離: Routerはマニフェスト提示、NodeがDLL/アーティファクト解決を担当。
- テスト: DLL未配置/アーティファクト欠落の明確なエラーを保証。

## プロジェクト構造
```
specs/SPEC-5f3dd53a/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
└── contracts/
    └── README.md
```

## Phase 0: Research
- gptoss_* C API のCUDA実装提供有無と配布元を確認
- DLLのライセンス/配布条件の整理
- CUDAアーティファクト命名の既存慣例を確認
- PoC (`poc/nemotron-cuda-cpp`) を DLL 化できるか評価

## Phase 1: Design
- DLL探索順序 (env -> model dir -> default) の確定
- CUDAアーティファクト配置規則の確定
- Nodeのready判定/エラー分類の整理

## Phase 2: Task planning (方針)
- Node: CUDA DLL解決/ロード処理
- Node: CUDAアーティファクト解決ロジック
- テスト: DLL未配置/アーティファクト欠落のエラー検証
- ドキュメント: 配置例/環境変数/運用手順

## 受け入れ条件
- gpt-oss/nemotronのCUDA DLLが配置済みであればモデルが ready になる
- DLLやCUDAアーティファクト不足時に明確なエラーとなる
- 環境変数指定とモデルディレクトリ配置の両方に対応する

## リスク/未確定事項
- CUDA DLLの提供元/ビルド手順が未確定
- CUDAアーティファクトの生成/配布経路が未確定
- gptoss_* API 互換の CUDA 実装が未整備
