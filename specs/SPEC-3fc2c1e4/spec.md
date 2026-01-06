# SPEC-3fc2c1e4: 実行エンジン�E�統合仕様！E

**スチE�Eタス**: 計画中�E�統合！E

## こ�E仕様�E役割�E��E見向け！E
こ�E仕様�E**実行エンジン領域の入口ガイチE*です、E 
エンジン選抁E抽象化�E原則をまとめ、詳細は下記�E個別SPECに委譲します、E

## 背景 / 問顁E
推論エンジンに関する要件が�E散し、モチE��管琁E��混在することで責務が曖昧になってぁE��、E
実行エンジン領域を統合し、モチE��管琁E��は明確に刁E��する忁E��がある、E

## 目皁E
- Node側エンジン抽象化と推論責務を統合的に定義する
- GPU前提�E�Eetal/CUDA�E��E実行要件を�E確化すめE
- エンジン選択が登録時�EアーチE��ファクトに従うことを保証する

## スコーチE
- Node側のエンジン抽象化！EngineRegistry/Engineの責務！E
- 実行環墁E�E前提�E�EPU忁E��！E
- マニフェストとローカル実体に基づくエンジン選抁E

## 非ゴール
- モチE��登録・保存（モチE��管琁E��域�E�E
- 自動変換/量子化生�E
- Nemotron推論エンジンの詳細設計！EBD�E�E

## 原則
- `metadata.json` のような独自メタチE�Eタには依存しなぁE
- エンジン選択�E「�Eニフェストとローカル実体」を正とする
- 形式選択�ERouterで行わず、Nodeがruntime/GPU要件に応じて判断する

## 決定事頁E���E有用サマリ�E�E
- **責務�E離**: 形式選択�ENode側で行い、ルーターはマニフェスト提供に徹する、E
- **Node前提**: Node は Python 依存を導�EしなぁE��E
- **GPU前提**: GPU 非搭載ノード�E対象外（登録不可�E�、E
- **対応OS/GPU**:
  - macOS: Apple Silicon�E�Eetal�E�E
  - Windows: CUDA
  - Linux: 当面は非対忁E
  - WSL2: 対象外！EindowsはネイチE��ブ�Eみ�E�E
- **CUDA��o�H�̗��R**: Windows�ł�CUDA�����萫/�Č����ŗD�ʁADirectML�̓A�[�e�B�t�@�N�g�s���ƃh���C�o�����̉e�����傫�����ߓ����i�����g�̂݁j�B
- **形式選抁E*: safetensors/GGUF 等�E **format は登録時に確宁E*し、実行時の刁E��は行わなぁE��E 
  NodeはGPUに応じて **公式最適化アーチE��ファクチE*�E�侁E Metal/CUDA向け�E�を選択する、E
- **最適化アーチE��ファクチE*: 公式最適化アーチE��ファクト�E利用優先�Eエンジン領域の実行最適化として扱ぁE��Nodeが選択したアーチE��ファクトを置き換えなぁE��E
- **Nemotron**: 新エンジンの仕槁E実裁E�E後回し！EBD�E�、E
- **冁E��エンジンの要件は単一匁E*: 詳細は「�E蔵エンジン要件�E�単一要件�E�」に統合済み、E

## 冁E��エンジン要件�E�単一要件�E�E

- **REQ-IE-001**: 冁E��エンジンは **RuntimeType/format/capabilities** に基づく単一の刁E��規紁E��持ち、E 
  LLM/Embedding�E�ELlamaCpp`,`GptOssCpp`,`NemotronCpp`�E�、ASR�E�EWhisperCpp`�E�、TTS�E�EOnnxRuntime`�E�、E 
  画像生成！EStableDiffusion`�E�、画像認識（新エンジン�E�として扱ぁE��E 
  **冁E��エンジンの具体侁E*は llama.cpp / gpt-oss / nemotron / whisper.cpp / stable-diffusion.cpp / ONNX Runtime とする、E 
  併せて、次の条件めE**1つの要件として**満たすこと:
  - **プラグイン形弁E*: Node 本体�E Engine Host とし、エンジンは動的プラグイン�E�Edylib/.so/.dll�E�で追加可能、E
  - **ABI固宁E*: プラグインは C ABI で互換性を保証し、`abi_version` を忁E��とする、E
  - **選択ソース**: 登録時に確定しぁE`format` と HF 由来メタチE�Eタ�E�Econfig.json` 等）を正とし、E 
    `metadata.json` のような独自メタチE�Eタには依存しなぁE��E
  - **自動フォールバック禁止**: safetensors/GGUF が�E存する場合�E登録時�E `format` 持E��が忁E��で、E 
    実行時の形式�E替は行わなぁE��E
  - **GPU前提**: エンジンは GPU 前提�E�EacOS=Metal / Windows=CUDA / Linux=Cudaは実験扱ぁE��、E
  - **可否判宁E*: こ�E刁E���E EngineRegistry/EngineHost および `/v1/models` の可否判定に反映され、E 
    **未対応カチE��リは登録対象から除夁E*される、E

## 冁E��エンジンのアーキチE��チャ�E�概念�E�E

> 目皁E **冁E��エンジン群の責務墁E��と選択フロー**を一枚で把握できるようにする、E

### 構�E図�E�概念�E�E

```
┌──────────────━E              ┌────────────────────────────────────━E
━E Router      ━E              ━E               Node                ━E
━E - 登録/メタ │──manifest────▶━E ModelStorage / Resolver            ━E
━E - HF検証    ━E              ━E - config/tokenizer検証             ━E
└──────────────━E              ━E - runtime確宁E                     ━E
                                ━E - 忁E��アーチE��ファクト選抁E        ━E
                                ━E - 外部ソース�E�EF等）から直接取征E  ━E
                                ━E            ━E
                                ━E            ▼
                                ━E    EngineRegistry
                                ━E (RuntimeTypeで選抁E
                                ━E            ━E
                                ━E            ▼
                                ━E Engine Host (Plugin Loader)
                                ━E   ├─ GGUF ↁEllama.cpp (plugin)
                                ━E   ├─ TTS  ↁEONNX Runtime (plugin)
                                ━E   └─ safetensors ↁE独自エンジン群 (plugins)
                                ━E         ├─ gpt-oss (Metal/CUDA)
                                ━E         ├─ nemotron (TBD)
                                ━E         └─ そ�E他！Ehisper/SD など�E�E
                                └────────────────────────────────────━E
```

### 主要コンポ�EネンチE

- **Router**
  - 登録時に **メタチE�Eタとマニフェスト（ファイル一覧�E�E* を保存する、E
  - **形式�E登録時に確宁E*し、Nodeはformatを尊重する�E�実行時の刁E��は行わなぁE��、E
  - ルーターはモチE��バイナリを保持しなぁE��E
- **Node / ModelStorage + Resolver**
  - 形式�Eファイルの整合性�E�Econfig.json` / `tokenizer.json` / shard / index�E�を検証、E
  - GPUバックエンドに応じて **忁E��アーチE��ファクトを選抁E*し、外部ソース�E�EF等）から直接取得する、E
  - `ModelDescriptor` を生成！Eormat / primary_path / runtime / capabilities�E�、E
- **EngineRegistry**
  - `RuntimeType` に基づぁE**外�Eの推論エンジンを確宁E*する、E
  - Node はマニフェストとローカル実体を正とし、E*実行時の自動変換は行わなぁE*、E
- **Inference Engine�E�外�E�E�E*
  - 共通�E推論インターフェース。�E部で runtime に応じてプラグインを振り�Eける、E
  - GGUF ↁE`llama.cpp`、TTS ↁE`ONNX Runtime`、safetensors ↁE独自エンジン群�E�すべてプラグイン�E�、E
  - **GGUFは llama.cpp が褁E��アーキチE��チャ�E�Elama/mistral/gemma/phi 等）を横断皁E��駁E��**する、E
  - **safetensorsは原則「モチE��ごとの専用エンジン、E*で対応する（侁E gpt-oss, nemotron�E�、E
    - 汎用safetensorsエンジンの可能性は否定しなぁE��、E*初期要件では前提にしなぁE*、E
  - 公式最適化アーチE��ファクト�E **実行キャチE��ュ**として利用可能だが、E
    Nodeが選択したアーチE��ファクト�E上書きしなぁE��E

### GPUバックエンド（最下層レイヤー�E�E

- **Metal**�E�EacOS / Apple Silicon�E�E
- **CUDA**�E�Eindows / NVIDIA�E�E
  - Linux: 当面は非対忁E

## 現状の対応済みモチE��/アーキチE��チャ�E�E026-01-02時点�E�E

- **GGUF / llama.cpp**: llama/mistral/qwen/gemma/phi 系が検証済み、E 
  詳細なモチE��IDと検証状況�E `specs/SPEC-6cd7f960/verified-models.md` を正とする、E
- **safetensors / 冁E��エンジン**: 実運用で確認できてぁE��のは **gpt-oss�E�Eetal/macOS�E�E* のみ、E 
  DirectMLは実験扱ぁE��NemotronはTBD�E�後回し）、E
- **そ�E他モダリチE��**�E�ESR/TTS/画像生戁E画像認譁EEmbedding�E�E  
  対応可否と検証済みモチE��は `specs/SPEC-6cd7f960/verified-models.md` で管琁E��る、E

### プラグイン設計指針！Eode�E�E

- **配币E��佁E*: 共有ライブラリ + manifest.json の 1 セチE��
- **manifest冁E��**:
  - engine_id / engine_version / abi_version
  - 対忁ERuntimeType / 形式！Eafetensors, gguf, onnx 等！E
  - 対忁Ecapabilities�E�Eext / vision / asr / tts / image�E�E
  - GPU 要件�E�Eetal / DirectML / CUDA(実騁E�E�E
- **互換性**: C ABI を固定し、ABI 互換を破る変更は abi_version を更新する
- **解決頁E��E*: EngineRegistry ぁERuntimeType と format をキーにプラグインを解決する
  - ベンチ�Eーク未設定�E場合、E*プラグイン�E�非builtin�E�を優允E*し、builtinはフォールバックとする

### RuntimeType とエンジンの対応（現状�E�E

| RuntimeType | 主用送E| 主要アーチE��ファクチE| 備老E|
|---|---|---|---|
| `LlamaCpp` | LLM / Embedding | GGUF | NodeがGGUFアーチE��ファクトを選択した場吁E|
| `GptOssCpp` | gpt-oss | safetensors + 公式最適匁E| macOSはMetal最適化、WindowsはCUDAを主経路 |
| `NemotronCpp` | Nemotron | safetensors | **TBD**�E�Eindows CUDA想定、Linux CUDAは実験扱ぁE��E|
| `WhisperCpp` | ASR | GGML/GGUF�E�当面�E�E| 変換は行わなぁE��safetensors対応�E封E��検訁E|
| `StableDiffusion` | 画像生戁E| safetensors�E�直接�E�E| stable-diffusion.cpp を当面利用 |
| `OnnxRuntime` | TTS | ONNX | Python依存なしで運用する |

**現状の実運用確誁E*
- safetensors系LLMで安定動作が確認できてぁE��のは **gpt-oss�E�Eetal/macOS�E�E* のみ、E
- Windows CUDAが主経路、DirectMLは限定的、NemotronはTBD�E�後回し）、E

### 現在の対応済みモチE���E�E026-01-02時点�E�E

**Model Hub�E�Erouter/src/supported_models.json`�E�に登録済み**の篁E��:

- **GGUF / llama.cpp**:
  - Qwen2.5 7B Instruct
  - Llama 3.2 3B Instruct
  - Mistral 7B Instruct
  - Phi-3 Mini
  - Gemma 2 9B
- **safetensors / gpt-oss**:
  - GPT-OSS 20B�E�Eetal�E�E
  - GPT-OSS 120B�E�Eetal�E�E
  - GPT-OSS Safeguard は **Metal最適化アーチE��ファクト未提侁E*のため未対忁E

詳細な検証状況�E `specs/SPEC-6cd7f960/verified-models.md` を参照、E

### アーチE��ファクト選択とエンジン選択�E原則

1. **Router は形式を確定せぁE*、�Eニフェスト�Eみを提供する、E
2. **Node がruntime/GPU要件に応じてアーチE��ファクトを選抁E*する、E
3. **変換は行わなぁE*�E�Eafetensors/GGUF/Metalはそ�Eまま扱ぁE��、E
4. **最適化アーチE��ファクト�E “実行キャチE��ュ E* として利用可能だが、E
   ローカル実体に存在しなぁE��合�EHFから直接取得する、E

## 性能/メモリ要件�E�測定と制紁E��E

- **測定タイミング**: モチE��登録時�Eエンジン更新時にベンチ�Eークを実行する、E
- **測定指樁E*: throughput�E�Eokens/sec�E�、TTFT、VRAM使用玁E��ピーク/平坁E��を記録する、E
- **測定条件**: コンチE��スト長めE��チE��サイズなどの条件をメタチE�Eタとして保持する、E
- **制紁E*: VRAM使用玁E��90%を趁E��、また�EOOMの場合�E失敗として扱ぁE��E
- **反映**: 測定結果は EngineRegistry の選択に反映する、E

### Nemotron の位置づぁE

- 冁E��エンジンの **一部として Nemotron 対応を含む**、E
- **Windows CUDA を想宁E*し、Linux CUDA は実験扱ぁE��Eetalは封E��対応）、E
- Nemotron 専用の詳細設計�E **TBD** として後段 SPEC に委譲、E
- **Windows CUDA (TBD)**: gptoss_* C API ??????DLL/ENV ? TBD?
- **DirectML (??)**: ??????????????`model.directml.bin` / `model.dml.bin` ??????

## 詳細仕様（参照�E�E
- **エンジン抽象匁E*: `SPEC-d7feaa2c`
- **gpt-oss-20b safetensors 実衁E*: `SPEC-2c0e5a9b`
- **gptossアーキチE��チャエイリアス**: `SPEC-8a2d1d43`
- **Nemotron PoC**: `SPEC-efff1da7`

## 受け入れ条件
1. Nodeのエンジン選択�E登録済みアーチE��ファクトとHFメタチE�Eタに一致する、E
2. GPU非搭載ノード�E対象外とする、E
3. モチE��管琁E�E仕様と矛盾しなぁE��E

## 依存関俁E
- `SPEC-08d2b908`�E�モチE��管琁E��合！E
- `SPEC-5cd7b614`�E�EPU忁E��ノード登録要件�E�E

---

## Clarifications

### Session 2025-12-30

インタビューにより以下が確宁E

**アーキチE��チャ基盤**:

- **プロセス刁E��**: 同一プロセス�E�モノリシチE���E�E
  - Engine HostとEngineプラグインは同一プロセス空間で動佁E
- **ビルド方弁E*: 動的リンク�E�EHARED ライブラリ�E�E
  - 吁E��ンジン�E�Elama.cpp、gptoss等）�E.so/.dylib/.dllとして個別ビルチE
  - `EngineHost`が`dlopen`/`LoadLibraryA`で動的ローチE
- **コンチE��スト設宁E*: モチE��定義に固定値
  - supported_models.json等でモチE��ごとにコンチE��スト長を定義

**エンジン選択戦略**:

- **ホットスワチE�E**: 不要E���E起動で刁E���E�E
  - ノ�Eド�E起動時のみエンジン構�Eを変更可能
- **優先頁E��決宁E*: 性能ベンチ�Eーク自動選抁E
  - 同一モチE��に褁E��エンジンが対応可能な場合、�Eンチ�Eーク結果で決宁E
- **VRAMアロケータ**: エンジン任せ！ES/ドライバ依存！E
  - 吁E��ンジンが独自にメモリ管琁E��競合時はOOMエラー
- **サードパーチE��プラグイン**: サポ�Eトする（制限なし！E
  - 任意�Eプラグインをディレクトリ配置でロード可能

**ベンチ�Eーク仕槁E*:

- **実行タイミング**: モチE��登録晁E
  - モチE��をsupported_modelsに登録する際にベンチ�Eーク実衁E
- **測定指樁E*: 褁E��スコア
  - スループット！Eokens/sec�E�E TTFT + VRAM使用玁E�E加重スコア
- **結果保孁E*: モチE��メタチE�Eタに埋め込み
  - supported_models.json等にベンチ�Eーク結果を追訁E
- **保存形式（暫定！E*:
  - モチE��メタチE�Eタ冁E��「engine_id ↁE褁E��スコア」�E対応を保持する
  - EngineRegistryはこ�Eスコアを参照して同一runtime冁E�Eエンジンを選択すめE
  - 参�EできなぁE��合�E登録頁E�E先頭エンジンを選択し、警告ログを残す

**エラーハンドリング**:

- **クラチE��ュ対忁E*: エンジン再ローチE
  - SEGFAULT等でクラチE��ュしたエンジンのみを�Eロード、他�E維持E
- **機�E未対忁E*: エラー返却�E�E01 Not Implemented�E�E
  - エンジンがサポ�EトしなぁE���E�E�Eision、function calling等）�Eエラー返却

**チE�Eタ管琁E*:

- **中間データエクスポ�EチE*: サポ�EトしなぁE
  - アチE��ション重み、アクチE��ベ�Eション等�E冁E��状態�E非�E閁E
- **KVキャチE��ュ共朁E*: 不要E
  - エンジン刁E��時�EコンチE��ストリセチE��、単純化を優允E

**プラグイン設計詳細**:

- **インスト�Eル方況E*: チE��レクトリ配置のみ
  - プラグインチE��レクトリに.so/.dylib + manifest.jsonを�E置
- **ABI不一致**: ロード拒否 + アラーチE
  - ダチE��ュボ�Eドやログで非互換プラグインを通知
- **カスタムパラメータ**: manifest.jsonに全て定義
  - エンジン固有設定�Emanifestに記述
- **モダリチE��統一**: 統一する
  - チE��スチE画僁E音声の全モダリチE��で同一Engineインターフェース
  - 入出力�EモダリチE��別メソチE���E�Eenerate_text(), generate_image()等）で刁E��

**ライフサイクル管琁E*:

- **アンロード�E琁E*: destroy + dlclose
  - エンジン破棁E��にライブラリもアンローチE
- **ログ統吁E*: 標準�E力キャプチャ
  - stdout/stderrをノードがキャプチャして統合ログへ
- **ヘルスチェチE��**: 不要E
  - プロアクチE��ブな確認なし、エラー時�Eみ検�E

**セキュリチE��・制陁E*:

- **タイムアウチE*: 不要E
  - エンジンを信頼、デチE��ロチE��検�EなぁE
- **ID競吁E*: ロードエラー
  - 同一engine_idの2つ目以降�Eロード拒否
- **メモリ制陁E*: 制限なぁE
  - エンジンは任意にRAM/VRAMを使用可能
- **ネットワーク**: 禁止
  - エンジンからの外部ネットワークアクセス�E�モチE��ダウンロード等）�E禁止

### Session 2025-12-30 (詳細インタビュー)

追加インタビューにより以下が確宁E

**障害検知と復旧**:

- **クラチE��ュ時�EリクエスチE*: 即座に500エラーを返却
  - クラチE��ュ時点で処琁E��のリクエストには500を返し、クライアントにリトライ判断を委�EめE
- **メモリリーク/VRAM枯渁E*: 閾値趁E��で自動�E起勁E
  - VRAM使用玁E��90%を趁E��た場合、エンジンを�E動的に再起勁E
- **ハング検知**: ウォチE��ドッグスレチE��
  - 別スレチE��からエンジンの応答性を監視、E0秒タイムアウトでハングと判宁E
- **ハング時�E対処**: ノ�Eド�E体を再起勁E
  - 同一プロセスのためエンジンのみの強制終亁E�E不可、ノード�Eロセス全体を再起勁E
- **VRAM OOM晁E*: エラー返却と継綁E
  - 該当リクエストにエラーを返し、エンジンは稼働継綁E

**バ�Eジョニングと互換性**:

- **更新検知**: manifest.jsonのバ�Eジョン番号で検知
  - ノ�Eド�E起動時にチE��レクトリをスキャン、バージョン変更を検�E
- **ベンチ�Eーク再実衁E*: エンジン更新時に自動�E実衁E
  - manifest.json更新検知時に関連モチE��の全ベンチ�Eークを�E動�E実衁E
- **ベンチ�Eーク実行タイミング**: リリース前�E手動チE��チE
  - ランタイムではなくCI/CDまた�Eリリース前に実行し、結果をsupported_models.jsonに埋め込み
- **バ�Eジョン共孁E*: 禁止
  - 同一engine_idの褁E��バ�Eジョンはロードエラー
- **ABIバ�Eジョン形弁E*: semver
  - "1.0.0"形式�EセマンチE��チE��バ�Eジョニング

**チE��チE��とトラブルシューチE��ング**:

- **エラーメチE��ージ形弁E*: パススルー
  - エンジン固有�EエラーメチE��ージをそのままクライアントに伝達
- **基本エラーコーチE*: 共通定義
  - OK/ERROR/OOM/TIMEOUTなど基本コード�Eみ共通、詳細はエンジン固朁E
- **ログレベル設宁E*: manifest.jsonに固宁E
  - 吁E��ンジンのチE��ォルトログレベルをmanifestで定義
- **エンジン惁E��API**: フルダンチE
  - manifest.jsonの冁E��をほぼそ�Eまま返却
- **エラー通知**: stdout/stderrをキャプチャ
  - ログ経由でエラーを検�E、リアルタイム通知は行わなぁE
- **ロード失敗通知**: ログのみ
  - GPUメモリ不足、依存ライブラリ不足等�Eログに記録、�E動的な通知なぁE

**実裁E��細**:

- **ト�EクンチE��ーチE*: エンジンがデコード済み斁E���Eを返す
  - エンジン冁E��でtokenizerを保持し、デコード済みチE��ストを返却
- **並行リクエスチE*: Engine Hostがキューイング
  - Hostがリクエストをキューに入れ、E��E��エンジンに投�E
- **ストリーミング**: コールバック関数
  - エンジンがトークンごとにHost提供�Eコールバックを呼ぶ
- **キャンセル方況E*: コールバックの戻り値
  - ストリーミングコールバックがfalseを返すとキャンセル
- **GPU別バイナリ**: GPU別に別バイナリ
  - llama_cpp_metal.dylib, llama_cpp_directml.dllのように刁E��
- **初期化コンチE��スチE*: 詳細惁E��
  - チE��イスID、VRAMサイズ、ドライババージョン、対応機�E等を提侁E
- **エンジンローチE*: 遁E��ローチE
  - 最初�Eリクエスト時に忁E��なエンジンをローチE
- **アンロード頁E��E*: LRU
  - 最も最近使われてぁE��ぁE��ンジンからアンローチE

**エチE��ケース処琁E*:

- **形式不一致検知**: モチE��登録晁E
  - 登録時にフォーマットとエンジンの対応を検証、不一致は拒否
- **GPU競合時**: 即座にエラー�E�E29 Too Many Requests�E�E
  - LLMがGPUを占有中に別モダリチE��のリクエストが来た場吁E
- **クライアント�E断**: 即座に中断
  - 推論を即時停止、E��刁E��果は返却しなぁE
- **max_tokens制陁E*: エンジンが�E琁E
  - max_tokensをエンジンに渡し、エンジンが停止判宁E
- **STOPト�Eクン**: Hostが�E琁E
  - Hostが�E力を監視し、STOP検知時にエンジンに停止持E��
- **usage形弁E*: エンジンがOpenAI形式で返す
  - {prompt_tokens, completion_tokens}形式で返却
- **vision入劁E*: 生バイト�E刁E
  - チE��ード済みのピクセルチE�Eタをエンジンに渡ぁE
- **Function Calling**: C構造体にマッチE
  - チE�Eル定義をABI定義の構造体にマッピング

**開発老E��騁E*:

- **SDK提侁E*: ヘッダー + チE��プレーチE
  - Cヘッダーファイルと最小限のスケルトンプロジェクトを提侁E
- **チE��ト方況E*: 提供しなぁE
  - サードパーチE��が独自にチE��ト環墁E��構篁E
- **署名検証**: なぁE
  - 署名検証なしでロード、ユーザーの自己責任
- **品質保証**: ユーザーの自己責任
  - サードパーチE��プラグインの品質はインスト�Eルしたユーザーが責任を持つ

**C ABIインターフェース**:

- **忁E��エクスポ�Eト関数**: フルセチE��
  - create/destroy/infer + load_model/unload_model/get_info + cancel/get_metrics/set_config
- **未対応モダリチE��**: NOT_SUPPORTEDを返すスタチE
  - 全関数をエクスポ�Eトし、未対応�Eエラーコードを返す
- **get_info()冁E��**: フル惁E��
  - engine_id, version, capabilities, gpu_backends, ロード済みモチE��一覧、メモリ使用量筁E

**チE��レクトリ構造**:

- **プラグイン配置**: GPU別サブディレクトリ
  - `engines/llama_cpp/metal/llama_cpp.dylib`
  - `engines/llama_cpp/directml/llama_cpp.dll`
  - `engines/llama_cpp/manifest.json`�E��E通！E

**manifest.json拡張**:

- **依存ライブラリ**: ライブラリ吁E+ バ�Eジョン
  - 忁E��な外部ライブラリの名前と最小バージョンを�E訁E
- **GPU表現**: 褁E��対応可能
  - `"gpu_backends": ["metal", "directml"]`のように配�Eで表現
- **モチE��パス**: 相対パス�E�モチE��チE��レクトリ基準！E
  - エンジンにはモチE��チE��レクトリからの相対パスを渡ぁE
- **エンコーチE��ング**: UTF-8固宁E
  - 全ての斁E���E入出力�EUTF-8を前揁E

**トレードオフと懸念事頁E*:

- **クラチE��ュリスク**: 封E��皁E��プロセス刁E��も検訁E
  - 同一プロセスによる道連れリスクは許容、安定性問題が出れ�E刁E��を検訁E
- **最大の懸念**: パフォーマンス
  - ABIオーバ�EヘッドやIPC相当�Eコストがボトルネックになる可能性
