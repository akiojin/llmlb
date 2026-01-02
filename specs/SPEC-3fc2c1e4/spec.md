# SPEC-3fc2c1e4: 実行エンジン（統合仕様）

**ステータス**: 計画中（統合）

## この仕様の役割（初見向け）
この仕様は**実行エンジン領域の入口ガイド**です。  
エンジン選択/抽象化の原則をまとめ、詳細は下記の個別SPECに委譲します。

## 背景 / 問題
推論エンジンに関する要件が分散し、モデル管理と混在することで責務が曖昧になっている。
実行エンジン領域を統合し、モデル管理とは明確に分離する必要がある。

## 目的
- Node側エンジン抽象化と推論責務を統合的に定義する
- GPU前提（Metal/DirectML）の実行要件を明確化する
- エンジン選択が登録時のアーティファクトに従うことを保証する

## スコープ
- Node側のエンジン抽象化（EngineRegistry/Engineの責務）
- 実行環境の前提（GPU必須）
- マニフェストとローカル実体に基づくエンジン選択

## 非ゴール
- モデル登録・保存（モデル管理領域）
- 自動変換/量子化生成
- Nemotron推論エンジンの詳細設計（TBD）

## 原則
- `metadata.json` のような独自メタデータには依存しない
- エンジン選択は「マニフェストとローカル実体」を正とする
- 形式選択はRouterで行わず、Nodeがruntime/GPU要件に応じて判断する

## 決定事項（共有用サマリ）
- **責務分離**: 形式選択はNode側で行い、ルーターはマニフェスト提供に徹する。
- **Node前提**: Node は Python 依存を導入しない。
- **GPU前提**: GPU 非搭載ノードは対象外（登録不可）。
- **対応OS/GPU**:
  - macOS: Apple Silicon（Metal）
  - Windows: DirectML（D3D12）を主経路とする
  - Linux: 当面は非対応（CUDAは実験扱い）
  - WSL2: 対象外（Windowsはネイティブのみ）
- **形式選択**: safetensors/GGUF/Metal 等の選択は Node が実行環境に応じて行う。
- **最適化アーティファクト**: 公式最適化アーティファクトの利用優先はエンジン領域の実行最適化として扱い、Nodeが選択したアーティファクトを置き換えない。
- **Nemotron**: 新エンジンの仕様/実装は後回し（TBD）。
- **内蔵エンジンの要件は単一化**: 詳細は「内蔵エンジン要件（単一要件）」に統合済み。

## 内蔵エンジン要件（単一要件）

- **REQ-IE-001**: 内蔵エンジンは **RuntimeType/format/capabilities** に基づく単一の分類規約を持ち、  
  LLM/Embedding（`LlamaCpp`,`GptOssCpp`,`NemotronCpp`）、ASR（`WhisperCpp`）、TTS（`OnnxRuntime`）、  
  画像生成（`StableDiffusion`）、画像認識（新エンジン）として扱う。  
  **内蔵エンジンの具体例**は llama.cpp / gpt-oss / nemotron / whisper.cpp / stable-diffusion.cpp / ONNX Runtime とする。  
  併せて、次の条件を **1つの要件として**満たすこと:
  - **プラグイン形式**: Node 本体は Engine Host とし、エンジンは動的プラグイン（.dylib/.so/.dll）で追加可能。
  - **ABI固定**: プラグインは C ABI で互換性を保証し、`abi_version` を必須とする。
  - **選択ソース**: 登録時に確定した `format` と HF 由来メタデータ（`config.json` 等）を正とし、  
    `metadata.json` のような独自メタデータには依存しない。
  - **自動フォールバック禁止**: safetensors/GGUF が共存する場合は登録時の `format` 指定が必須で、  
    実行時の形式切替は行わない。
  - **GPU前提**: エンジンは GPU 前提（macOS=Metal / Windows=DirectML / Linux=Cudaは実験扱い）。
  - **可否判定**: この分類は EngineRegistry/EngineHost および `/v1/models` の可否判定に反映され、  
    **未対応カテゴリは登録対象から除外**される。

## 内蔵エンジンのアーキテクチャ（概念）

> 目的: **内蔵エンジン群の責務境界と選択フロー**を一枚で把握できるようにする。

### 構成図（概念）

```
┌──────────────┐               ┌────────────────────────────────────┐
│  Router      │               │                Node                │
│  - 登録/メタ │──manifest────▶│  ModelStorage / Resolver            │
│  - HF検証    │               │  - config/tokenizer検証             │
└──────────────┘               │  - runtime確定                      │
                                │  - 必要アーティファクト選択         │
                                │  - 外部ソース（HF等）から直接取得   │
                                │             │
                                │             ▼
                                │     EngineRegistry
                                │  (RuntimeTypeで選択)
                                │             │
                                │             ▼
                                │  Engine Host (Plugin Loader)
                                │    ├─ GGUF → llama.cpp (plugin)
                                │    ├─ TTS  → ONNX Runtime (plugin)
                                │    └─ safetensors → 独自エンジン群 (plugins)
                                │          ├─ gpt-oss (Metal/DirectML)
                                │          ├─ nemotron (TBD)
                                │          └─ その他（Whisper/SD など）
                                └────────────────────────────────────┘
```

### 主要コンポーネント

- **Router**
  - 登録時に **メタデータとマニフェスト（ファイル一覧）** を保存する。
  - **形式はRouterで確定しない**。Nodeがruntime/GPU要件に応じて選択する。
  - ルーターはモデルバイナリを保持しない。
- **Node / ModelStorage + Resolver**
  - 形式・ファイルの整合性（`config.json` / `tokenizer.json` / shard / index）を検証。
  - GPUバックエンドに応じて **必要アーティファクトを選択**し、外部ソース（HF等）から直接取得する。
  - `ModelDescriptor` を生成（format / primary_path / runtime / capabilities）。
- **EngineRegistry**
  - `RuntimeType` に基づき **外側の推論エンジンを確定**する。
  - Node はマニフェストとローカル実体を正とし、**実行時の自動変換は行わない**。
- **Inference Engine（外側）**
  - 共通の推論インターフェース。内部で runtime に応じてプラグインを振り分ける。
  - GGUF → `llama.cpp`、TTS → `ONNX Runtime`、safetensors → 独自エンジン群（すべてプラグイン）。
  - **GGUFは llama.cpp が複数アーキテクチャ（llama/mistral/gemma/phi 等）を横断的に駆動**する。
  - **safetensorsは原則「モデルごとの専用エンジン」**で対応する（例: gpt-oss, nemotron）。
    - 汎用safetensorsエンジンの可能性は否定しないが、**初期要件では前提にしない**。
  - 公式最適化アーティファクトは **実行キャッシュ**として利用可能だが、
    Nodeが選択したアーティファクトは上書きしない。

### GPUバックエンド（最下層レイヤー）

- **Metal**（macOS / Apple Silicon）
- **DirectML**（Windows / D3D12）
- **CUDA**（Linux: 実験扱い）

### プラグイン設計指針（Node）

- **配布単位**: 共有ライブラリ + manifest.json の 1 セット
- **manifest内容**:
  - engine_id / engine_version / abi_version
  - 対応 RuntimeType / 形式（safetensors, gguf, onnx 等）
  - 対応 capabilities（text / vision / asr / tts / image）
  - GPU 要件（Metal / DirectML / CUDA(実験)）
- **互換性**: C ABI を固定し、ABI 互換を破る変更は abi_version を更新する
- **解決順序**: EngineRegistry が RuntimeType と format をキーにプラグインを解決する
  - ベンチマーク未設定の場合、**プラグイン（非builtin）を優先**し、builtinはフォールバックとする

### RuntimeType とエンジンの対応（現状）

| RuntimeType | 主用途 | 主要アーティファクト | 備考 |
|---|---|---|---|
| `LlamaCpp` | LLM / Embedding | GGUF | NodeがGGUFアーティファクトを選択した場合 |
| `GptOssCpp` | gpt-oss | safetensors + 公式最適化 | macOSはMetal最適化、WindowsはDirectMLを主経路 |
| `NemotronCpp` | Nemotron | safetensors | **TBD**（Windows DirectML想定、Linux CUDAは実験扱い） |
| `WhisperCpp` | ASR | GGML/GGUF（当面） | 変換は行わない。safetensors対応は将来検討 |
| `StableDiffusion` | 画像生成 | safetensors（直接） | stable-diffusion.cpp を当面利用 |
| `OnnxRuntime` | TTS | ONNX | Python依存なしで運用する |

**現状の実運用確認**
- safetensors系LLMで安定動作が確認できているのは **gpt-oss（Metal/macOS）** のみ。
- DirectMLは限定的、NemotronはTBD（後回し）。

### アーティファクト選択とエンジン選択の原則

1. **Router は形式を確定せず**、マニフェストのみを提供する。
2. **Node がruntime/GPU要件に応じてアーティファクトを選択**する。
3. **変換は行わない**（safetensors/GGUF/Metalはそのまま扱う）。
4. **最適化アーティファクトは “実行キャッシュ”** として利用可能だが、
   ローカル実体に存在しない場合はHFから直接取得する。

## 性能/メモリ要件（測定と制約）

- **測定タイミング**: モデル登録時・エンジン更新時にベンチマークを実行する。
- **測定指標**: throughput（tokens/sec）、TTFT、VRAM使用率（ピーク/平均）を記録する。
- **測定条件**: コンテキスト長やバッチサイズなどの条件をメタデータとして保持する。
- **制約**: VRAM使用率が90%を超過、またはOOMの場合は失敗として扱う。
- **反映**: 測定結果は EngineRegistry の選択に反映する。

### Nemotron の位置づけ

- 内蔵エンジンの **一部として Nemotron 対応を含む**。
- **Windows DirectML を想定**し、Linux CUDA は実験扱い（Metalは将来対応）。
- Nemotron 専用の詳細設計は **TBD** として後段 SPEC に委譲。

## 詳細仕様（参照）
- **エンジン抽象化**: `SPEC-d7feaa2c`
- **gpt-oss-20b safetensors 実行**: `SPEC-2c0e5a9b`
- **gptossアーキテクチャエイリアス**: `SPEC-8a2d1d43`
- **Nemotron PoC**: `SPEC-efff1da7`

## 受け入れ条件
1. Nodeのエンジン選択は登録済みアーティファクトとHFメタデータに一致する。
2. GPU非搭載ノードは対象外とする。
3. モデル管理の仕様と矛盾しない。

## 依存関係
- `SPEC-08d2b908`（モデル管理統合）
- `SPEC-5cd7b614`（GPU必須ノード登録要件）

---

## Clarifications

### Session 2025-12-30

インタビューにより以下が確定:

**アーキテクチャ基盤**:

- **プロセス分離**: 同一プロセス（モノリシック）
  - Engine HostとEngineプラグインは同一プロセス空間で動作
- **ビルド方式**: 動的リンク（SHARED ライブラリ）
  - 各エンジン（llama.cpp、gptoss等）は.so/.dylib/.dllとして個別ビルド
  - `EngineHost`が`dlopen`/`LoadLibraryA`で動的ロード
- **コンテキスト設定**: モデル定義に固定値
  - supported_models.json等でモデルごとにコンテキスト長を定義

**エンジン選択戦略**:

- **ホットスワップ**: 不要（再起動で切替）
  - ノード再起動時のみエンジン構成を変更可能
- **優先順位決定**: 性能ベンチマーク自動選択
  - 同一モデルに複数エンジンが対応可能な場合、ベンチマーク結果で決定
- **VRAMアロケータ**: エンジン任せ（OS/ドライバ依存）
  - 各エンジンが独自にメモリ管理、競合時はOOMエラー
- **サードパーティプラグイン**: サポートする（制限なし）
  - 任意のプラグインをディレクトリ配置でロード可能

**ベンチマーク仕様**:

- **実行タイミング**: モデル登録時
  - モデルをsupported_modelsに登録する際にベンチマーク実行
- **測定指標**: 複合スコア
  - スループット（tokens/sec）+ TTFT + VRAM使用率の加重スコア
- **結果保存**: モデルメタデータに埋め込み
  - supported_models.json等にベンチマーク結果を追記
- **保存形式（暫定）**:
  - モデルメタデータ内に「engine_id → 複合スコア」の対応を保持する
  - EngineRegistryはこのスコアを参照して同一runtime内のエンジンを選択する
  - 参照できない場合は登録順の先頭エンジンを選択し、警告ログを残す

**エラーハンドリング**:

- **クラッシュ対応**: エンジン再ロード
  - SEGFAULT等でクラッシュしたエンジンのみを再ロード、他は維持
- **機能未対応**: エラー返却（501 Not Implemented）
  - エンジンがサポートしない機能（vision、function calling等）はエラー返却

**データ管理**:

- **中間データエクスポート**: サポートしない
  - アテンション重み、アクティベーション等の内部状態は非公開
- **KVキャッシュ共有**: 不要
  - エンジン切替時はコンテキストリセット、単純化を優先

**プラグイン設計詳細**:

- **インストール方法**: ディレクトリ配置のみ
  - プラグインディレクトリに.so/.dylib + manifest.jsonを配置
- **ABI不一致**: ロード拒否 + アラート
  - ダッシュボードやログで非互換プラグインを通知
- **カスタムパラメータ**: manifest.jsonに全て定義
  - エンジン固有設定はmanifestに記述
- **モダリティ統一**: 統一する
  - テキスト/画像/音声の全モダリティで同一Engineインターフェース
  - 入出力はモダリティ別メソッド（generate_text(), generate_image()等）で分離

**ライフサイクル管理**:

- **アンロード処理**: destroy + dlclose
  - エンジン破棄後にライブラリもアンロード
- **ログ統合**: 標準出力キャプチャ
  - stdout/stderrをノードがキャプチャして統合ログへ
- **ヘルスチェック**: 不要
  - プロアクティブな確認なし、エラー時のみ検出

**セキュリティ・制限**:

- **タイムアウト**: 不要
  - エンジンを信頼、デッドロック検出なし
- **ID競合**: ロードエラー
  - 同一engine_idの2つ目以降はロード拒否
- **メモリ制限**: 制限なし
  - エンジンは任意にRAM/VRAMを使用可能
- **ネットワーク**: 禁止
  - エンジンからの外部ネットワークアクセス（モデルダウンロード等）は禁止

### Session 2025-12-30 (詳細インタビュー)

追加インタビューにより以下が確定:

**障害検知と復旧**:

- **クラッシュ時のリクエスト**: 即座に500エラーを返却
  - クラッシュ時点で処理中のリクエストには500を返し、クライアントにリトライ判断を委ねる
- **メモリリーク/VRAM枯渇**: 閾値超過で自動再起動
  - VRAM使用率が90%を超えた場合、エンジンを自動的に再起動
- **ハング検知**: ウォッチドッグスレッド
  - 別スレッドからエンジンの応答性を監視、30秒タイムアウトでハングと判定
- **ハング時の対処**: ノード全体を再起動
  - 同一プロセスのためエンジンのみの強制終了は不可、ノードプロセス全体を再起動
- **VRAM OOM時**: エラー返却と継続
  - 該当リクエストにエラーを返し、エンジンは稼働継続

**バージョニングと互換性**:

- **更新検知**: manifest.jsonのバージョン番号で検知
  - ノード再起動時にディレクトリをスキャン、バージョン変更を検出
- **ベンチマーク再実行**: エンジン更新時に自動再実行
  - manifest.json更新検知時に関連モデルの全ベンチマークを自動再実行
- **ベンチマーク実行タイミング**: リリース前の手動テスト
  - ランタイムではなくCI/CDまたはリリース前に実行し、結果をsupported_models.jsonに埋め込み
- **バージョン共存**: 禁止
  - 同一engine_idの複数バージョンはロードエラー
- **ABIバージョン形式**: semver
  - "1.0.0"形式のセマンティックバージョニング

**デバッグとトラブルシューティング**:

- **エラーメッセージ形式**: パススルー
  - エンジン固有のエラーメッセージをそのままクライアントに伝達
- **基本エラーコード**: 共通定義
  - OK/ERROR/OOM/TIMEOUTなど基本コードのみ共通、詳細はエンジン固有
- **ログレベル設定**: manifest.jsonに固定
  - 各エンジンのデフォルトログレベルをmanifestで定義
- **エンジン情報API**: フルダンプ
  - manifest.jsonの内容をほぼそのまま返却
- **エラー通知**: stdout/stderrをキャプチャ
  - ログ経由でエラーを検出、リアルタイム通知は行わない
- **ロード失敗通知**: ログのみ
  - GPUメモリ不足、依存ライブラリ不足等はログに記録、能動的な通知なし

**実装詳細**:

- **トークンデコード**: エンジンがデコード済み文字列を返す
  - エンジン内部でtokenizerを保持し、デコード済みテキストを返却
- **並行リクエスト**: Engine Hostがキューイング
  - Hostがリクエストをキューに入れ、順次エンジンに投入
- **ストリーミング**: コールバック関数
  - エンジンがトークンごとにHost提供のコールバックを呼ぶ
- **キャンセル方法**: コールバックの戻り値
  - ストリーミングコールバックがfalseを返すとキャンセル
- **GPU別バイナリ**: GPU別に別バイナリ
  - llama_cpp_metal.dylib, llama_cpp_directml.dllのように分離
- **初期化コンテキスト**: 詳細情報
  - デバイスID、VRAMサイズ、ドライババージョン、対応機能等を提供
- **エンジンロード**: 遅延ロード
  - 最初のリクエスト時に必要なエンジンをロード
- **アンロード順序**: LRU
  - 最も最近使われていないエンジンからアンロード

**エッジケース処理**:

- **形式不一致検知**: モデル登録時
  - 登録時にフォーマットとエンジンの対応を検証、不一致は拒否
- **GPU競合時**: 即座にエラー（429 Too Many Requests）
  - LLMがGPUを占有中に別モダリティのリクエストが来た場合
- **クライアント切断**: 即座に中断
  - 推論を即時停止、部分結果は返却しない
- **max_tokens制限**: エンジンが処理
  - max_tokensをエンジンに渡し、エンジンが停止判定
- **STOPトークン**: Hostが処理
  - Hostが出力を監視し、STOP検知時にエンジンに停止指示
- **usage形式**: エンジンがOpenAI形式で返す
  - {prompt_tokens, completion_tokens}形式で返却
- **vision入力**: 生バイト配列
  - デコード済みのピクセルデータをエンジンに渡す
- **Function Calling**: C構造体にマップ
  - ツール定義をABI定義の構造体にマッピング

**開発者体験**:

- **SDK提供**: ヘッダー + テンプレート
  - Cヘッダーファイルと最小限のスケルトンプロジェクトを提供
- **テスト方法**: 提供しない
  - サードパーティが独自にテスト環境を構築
- **署名検証**: なし
  - 署名検証なしでロード、ユーザーの自己責任
- **品質保証**: ユーザーの自己責任
  - サードパーティプラグインの品質はインストールしたユーザーが責任を持つ

**C ABIインターフェース**:

- **必須エクスポート関数**: フルセット
  - create/destroy/infer + load_model/unload_model/get_info + cancel/get_metrics/set_config
- **未対応モダリティ**: NOT_SUPPORTEDを返すスタブ
  - 全関数をエクスポートし、未対応はエラーコードを返す
- **get_info()内容**: フル情報
  - engine_id, version, capabilities, gpu_backends, ロード済みモデル一覧、メモリ使用量等

**ディレクトリ構造**:

- **プラグイン配置**: GPU別サブディレクトリ
  - `engines/llama_cpp/metal/llama_cpp.dylib`
  - `engines/llama_cpp/directml/llama_cpp.dll`
  - `engines/llama_cpp/manifest.json`（共通）

**manifest.json拡張**:

- **依存ライブラリ**: ライブラリ名 + バージョン
  - 必要な外部ライブラリの名前と最小バージョンを明記
- **GPU表現**: 複数対応可能
  - `"gpu_backends": ["metal", "directml"]`のように配列で表現
- **モデルパス**: 相対パス（モデルディレクトリ基準）
  - エンジンにはモデルディレクトリからの相対パスを渡す
- **エンコーディング**: UTF-8固定
  - 全ての文字列入出力はUTF-8を前提

**トレードオフと懸念事項**:

- **クラッシュリスク**: 将来的にプロセス分離も検討
  - 同一プロセスによる道連れリスクは許容、安定性問題が出れば分離を検討
- **最大の懸念**: パフォーマンス
  - ABIオーバーヘッドやIPC相当のコストがボトルネックになる可能性
