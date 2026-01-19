# 機能仕様書: Windows CUDA runtime DLL (gpt-oss/nemotron)

**機能ID**: `SPEC-5f3dd53a`
**作成日**: 2026-01-06
**ステータス**: 下書き
**入力**: ユーザー説明 "gptoss_cuda.dll / nemotron_cuda.dll が必要なら作成するべき。仕様書がないため仕様を作成する。"

## ユーザーシナリオ / ユースケース (必須)

### ユーザーストーリー1 - CUDA DLLを配置して推論を有効化したい (優先度: P1)

運用管理者として、Windows CUDAノードで gpt-oss / nemotron の safetensors 推論を行うために、CUDA用ランタイムDLLを配置・指定し、モデルが ready になることを期待する。

**この優先度の理由**: Windows CUDA主経路を成立させる前提となるため。

**独立テスト**: DLL配置とCUDAアーティファクトが揃ったモデルを登録し、/v1/models が ready を返すことを確認する。

**受け入れシナリオ**:

1. **前提** Windows CUDAノードに `gptoss_cuda.dll` が配置済み、**実行** gpt-oss safetensors モデルを登録、**結果** `/v1/models` に ready で表示される。
2. **前提** Windows CUDAノードに `nemotron_cuda.dll` が配置済み、**実行** nemotron safetensors モデルを登録、**結果** `/v1/models` に ready で表示される。

---

### ユーザーストーリー2 - DLLやアーティファクト不足を明確に知りたい (優先度: P1)

運用管理者として、CUDA DLLや必要なアーティファクトが不足している場合に、明確なエラーで原因を把握したい。

**この優先度の理由**: 導入・運用の失敗原因が不明だと復旧が難しいため。

**独立テスト**: DLL未配置・アーティファクト欠落時のエラーを確認する。

**受け入れシナリオ**:

1. **前提** CUDA DLL未配置、**実行** モデル登録、**結果** DLL不足が明示されたエラーで失敗する。
2. **前提** DLLはあるがCUDAアーティファクトが欠落、**実行** モデル登録、**結果** アーティファクト不足が明示されたエラーで失敗する。

---

### ユーザーストーリー3 - DLL配置の選択肢を持ちたい (優先度: P2)

運用管理者として、モデルディレクトリ配置か環境変数指定のどちらでもDLLを指定できるようにしたい。

**この優先度の理由**: 配布・運用形態が環境ごとに異なるため。

**独立テスト**: 環境変数指定とモデルディレクトリ配置の両方でロードできることを確認する。

**受け入れシナリオ**:

1. **前提** `ALLM_GPTOSS_CUDA_LIB` が設定済み、**実行** モデル登録、**結果** gpt-oss CUDA DLL をロードできる。
2. **前提** `ALLM_NEMOTRON_CUDA_LIB` が設定済み、**実行** モデル登録、**結果** nemotron CUDA DLL をロードできる。

---

### エッジケース

- DLLが見つからない場合、どの探索経路で失敗したかが分かるエラーを返す。
- CUDAアーティファクトが存在しない場合、モデルは ready にならない。
- DirectMLのみが利用可能な環境では CUDA を要求しない（DirectMLは本仕様の対象外）。

## 実装方針 (補足)

- CUDA DLL は本リポジトリ内の `node/engines/gptoss/cuda/` と `node/engines/nemotron/cuda/` で管理し、CMake で `gptoss_cuda.dll` / `nemotron_cuda.dll` を生成する。
- `poc/` は参考用であり、仕様・実装の正とはしない。

## 要件 (必須)

### 機能要件

- **FR-001**: NodeはWindows CUDA向けに `gptoss_cuda.dll` をロードできる。
- **FR-002**: NodeはWindows CUDA向けに `nemotron_cuda.dll` をロードできる。
- **FR-003**: DLL探索は以下の優先順で行う: 環境変数指定 -> モデルディレクトリ -> 既定の検索パス。
- **FR-004**: CUDAアーティファクトは `model.cuda.bin` または `cuda/model.bin` を優先して扱う。
- **FR-005**: DLLまたはアーティファクトが不足する場合、原因が分かるエラーを返す。

### 主要エンティティ

- **CudaRuntimeDll**: gpt-oss / nemotron 用のCUDAランタイムDLL。
- **CudaArtifact**: `model.cuda.bin` / `cuda/model.bin` に配置されるCUDA最適化アーティファクト。

---

## スコープ外 (オプション)

- CUDAカーネルの実装や最適化。
- DirectMLの実行経路（凍結扱い）。
- Linux CUDAサポート。

---

## 技術制約 (該当する場合)

- Windows CUDA環境を前提とする。
- 既存の gptoss_* C API と互換のABIを維持する。

---

## 前提条件 (該当する場合)

- CUDA対応GPUとCUDAドライバがインストール済みである。
- gpt-oss / nemotron の safetensors メタデータが揃っている。

---

## 依存関係 (該当する場合)

- `SPEC-d7feaa2c` (エンジンローダー)
- `SPEC-2c0e5a9b` (gpt-oss safetensors 実行)
- `SPEC-3fc2c1e4` (実行エンジン統合仕様)

---

## 成功基準 (必須)

1. Windows CUDAノードで gpt-oss / nemotron のモデルが ready になる。
2. DLLやCUDAアーティファクト不足時に明確なエラーが返る。
3. 環境変数指定とモデルディレクトリ配置の両方でDLLをロードできる。
