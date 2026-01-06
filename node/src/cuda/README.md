# CUDA runtime DLLs (gpt-oss / nemotron)

このディレクトリは Windows CUDA 向けの `gptoss_cuda.dll` / `nemotron_cuda.dll` の**管理ソース**です。

- PoC (`poc/`) は参考用であり、**仕様・実装の正**ではありません。
- 実装は段階的に置き換える前提で、まずは **DLL を本リポジトリ内で管理**します。
- 現時点の CUDA 実装はスタブ（未実装）で、API 呼び出しは `gptoss_status_unsupported_system` を返します。

ログ確認: `LLM_NODE_CUDA_RUNTIME_LOG=1`
