# SPEC-d7feaa2c: Plan

## 方針
- Node側にエンジン抽象化レイヤーを導入し、実行エンジンを差し替え可能にする
- 内蔵エンジンは **プラグイン形式（動的ロード）** とし、C ABI固定で互換性を担保する
- 既存 llama.cpp は plugin として提供する
- エンジン選択は「登録時に選択したアーティファクト（safetensors/GGUF）」と
  Hugging Face の `config.json` 等のモデル由来メタデータを正として判定する
- `metadata.json` のような llm-router 独自メタデータファイルは使用しない
- Nemotron向けの新エンジン（推論エンジン）の仕様/実装は別SPECで後日決定（TBD）

## 設計スコープ（Router/Nodeの責務）

### Router（登録・配布）
- モデル登録時に「どのアーティファクトを使うか」を確定し、Nodeが迷わない形で配布する
  - safetensors/GGUF が両方ある場合は登録時に必須選択（`format`）
  - 選択された形式に応じて、配布マニフェスト（必要ファイル一覧）を確定する
- Node同期用API（`/v0/models` と `/v0/models/registry/...`）は「登録時に確定したもの」をそのまま返す
- llm-router独自の `metadata.json` は生成・参照しない

### Node（取得・検証・実行）
- Routerのマニフェストに従い、必要ファイルをローカルに取得（または共有パスを直接参照）する
- ローカル配置されたモデルを、`config.json` 等のHF由来メタデータで検証し、実行エンジンを決定する
- エンジンが未対応のモデルは「利用可能モデル一覧」から除外できる
- EngineHost（Plugin Loader）が runtime/format に一致する plugin をロードする

## エンジン選択（判定ソース）
- 判定の正は以下のみ
  1) 登録時に選択されたアーティファクト形式（safetensors/GGUF）
  2) Hugging Faceスナップショットのメタデータ（`config.json` など）
- ローカルに両形式が同居する前提の自動フォールバックは行わない（登録時に選択しているため）

## safetensorsモデルの取り扱い（ロード前提）
- `config.json` と `tokenizer.json` は必須（不足は登録時または同期時にエラー）
- 重みは以下のいずれかを「primary」として扱う
  - `*.safetensors.index.json`（シャーディングあり）
  - `*.safetensors`（単一ファイルのみ許可）
- `index` が無いのに `*.safetensors` が複数ある場合は曖昧としてエラー（どれが正本か決められない）

## GGUFモデルの取り扱い（フォールバック用途）
- GGUFは「正本」ではなく実行キャッシュとして扱う
- 登録時にGGUFファイル（または選択ポリシー）を確定し、Nodeは `model.gguf` として実行する

## 実装概要（Node側抽象化レイヤー）

### 1) Node側抽象化
- `EngineHost`（Plugin Loader）でプラグインをロード
- `EngineRegistry` で runtime ID → plugin を解決
- `Engine` は C ABI で提供し、manifest/abi_version を検証する

### 2) ModelStorage拡張
- Routerが配布したローカルのモデル配置（= 登録時の選択結果を反映）と
  `config.json` 等のメタデータを読み取り、`ModelDescriptor` として返す
- `listAvailable()` は「選択されたアーティファクトがローカルに存在するか」と
  「対応エンジンがあるか」で有効モデルを列挙する

### 3) Router側最小対応
- Nodeが必要とする「登録時の選択情報」を永続化し、Nodeへ渡せるようにする
- `metadata.json` を生成・参照しない

## テスト
- ModelStorage: `format` / 必須メタデータの検証テスト
- Engine selection: 登録時選択と `config.json` に基づく判定テスト
- Plugin loader: manifest/ABIバージョン検証テスト

## 互換性・保留（TBD）

### chat_template（完全互換）
- Nemotronを含むHFモデルの `chat_template` はJinja互換が前提のため、完全互換の扱いが必要
- ただし Nemotron向けの推論エンジンは後回し（別SPEC）なので、本SPECでは「要求の明記」に留める
  - 推奨案: Router（Rust）でJinja互換レンダリングを実装し、Nodeには「最終プロンプト」を渡す
    - 理由: Node（C++）にJinja完全互換実装を持ち込むと依存・ビルドが重くなるため
    - 位置づけ: ルーティング前処理（プロンプト形成）と、推論（GPU実行）を分離する
  - 互換性の目標: Hugging Face Transformers の chat_template（Jinja2）と同等のレンダリング結果
    - `messages` / `add_generation_prompt` などの標準変数を提供
    - token系（`bos_token`/`eos_token` 等）は `tokenizer.json` 由来の値を用いる
    - `chat_template` が無い場合はデフォルトテンプレート（例: ChatML）へフォールバック
  - 生成結果のパース: テンプレート由来の制御トークン（例: ChatMLやchannel marker）を除去し、最終メッセージを抽出する

### Metal/DirectML（GPU実行）
- NodeはGPU必須（Apple Silicon/Metal、Windows/DirectMLを主対象）
- 具体的なカーネル実装・dtype戦略は別SPECで決める（CUDAは実験扱い）

## Nemotron GPU PoC（後回し / TBD）

### 目的
- safetensors（正本）を直接読み込み、Metal/DirectMLのGPUで「実推論できる」ことを実証する
  - ここでの「実推論」は、最低でも 1トークン以上の生成が再現できること

### 段階（PoCの定義）
1. **重みの直読**: safetensors-cppでmmapし、index/shard構成でも破綻なく参照できる
2. **メタデータ整合**: `config.json` と `tokenizer.json` が揃い、必要なハイパーパラメータが解釈できる
3. **CPU参照実装（最小）**: 代表的な演算（matmul/norm/attention/MLP）をCPUで通せる（小モデルでOK）
4. **GPU実装（最小）**: Metal/DirectMLで主要演算を実装し、同一入力でCPU参照と一致（または許容誤差内）
5. **E2E生成**: 実際のトークナイズ→生成→デトークナイズまで一連で動作

### 現状（本ブランチでの到達点）
- safetensors-cpp による「重み直読PoC（テンソル列挙・検証）」まで
- GPU推論PoCは Nemotron推論エンジンと一体のため、別SPECで後日実施

---

## Session 2025-12-31 追加設計

### EngineHost拡張（ホットリロード・リソース監視）

**ホットリロード機能**:

1. 新プラグインをロード（シャドウロード）
2. 新規リクエストを新プラグインに振り分け
3. 旧プラグインの処理中リクエストが完了するまで待機
4. 旧プラグインをアンロード

**リソース監視機能**:

- VRAM使用率をポーリング（1秒間隔）
- RAM使用率をポーリング（1秒間隔）
- 90%閾値超過時にアラート + LRUアンロード

**VRAM割当管理**:

- プラグインロード時に必要VRAM量を申告させる
- 利用可能VRAMと比較し、不足時はロード拒否
- 複数プラグイン間のVRAM配分はホストが決定

### KVキャッシュ設計

- リクエストごとに独立したKVキャッシュを確保
- キャッシュサイズはコンテキスト長に応じて動的確保
- リクエスト完了時に即座に解放

### エラーコード体系

粗粒度エラーコード（約10種類）:

| コード | 名前 | 説明 |
|--------|------|------|
| 0 | OK | 成功 |
| 1 | OOM_VRAM | VRAM不足 |
| 2 | OOM_RAM | RAM不足 |
| 3 | MODEL_CORRUPT | モデルファイル破損 |
| 4 | TIMEOUT | タイムアウト |
| 5 | CANCELLED | キャンセル |
| 6 | UNSUPPORTED | 未サポート機能 |
| 7 | INTERNAL | 内部エラー |
| 8 | ABI_MISMATCH | ABIバージョン不一致 |
| 9 | LOAD_FAILED | ロード失敗 |

各コードに対して詳細なメッセージ文字列を付与。

### マルチGPU負荷分散設計

**GPU選択アルゴリズム**:

1. 利用可能なGPUを列挙
2. 各GPUのVRAM空き容量を取得
3. リクエスト到着時に最も空きの多いGPUを選択
4. モデルが既にロード済みのGPUがあれば優先

### 継続バッチング設計

**バッチスケジューラ**:

- Prefillキュー: 新規リクエストのprefill処理待ち
- Decodeバッチ: 生成中リクエストのdecode処理
- Prefill/Decode分離により、異なるプロンプト長の混合を回避

**バッチ追加タイミング**:

- decodeステップ間で新リクエストをバッチに追加
- KVキャッシュの動的確保

### メトリクス収集設計

**コールバックAPI**:

```
typedef void (*on_token_callback)(
    void* ctx,
    uint32_t token_id,
    uint64_t timestamp_ns
);
```

- プラグインはトークン生成ごとにコールバック
- ホスト側でTTFT、トークン/秒を計算

### プラグイン定期再起動設計

**再起動ポリシー**:

- 条件: N時間経過 または Nリクエスト処理後
- 再起動時はシャドウロード方式で無停止
- デフォルト値: TBD（運用で調整）

### manifest.jsonスキーマ拡張

```json
{
  "id": "llama_cpp",
  "version": "1.0.0",
  "abi_version": "1.0",
  "gpu_backend": "metal",
  "architectures": ["llama", "mistral", "gemma", "phi"],
  "binary": "libllama_engine.dylib"
}
```

- `architectures`: サポートするモデルアーキテクチャの列挙（必須）

---

## Session 2025-12-31 追加設計 Part 3

### 推論キャッシュ設計

**キャッシュ構造**:

- キー: (モデルID, プロンプトハッシュ, 温度, その他パラメータ)
- 値: 生成結果（トークン列）
- 温度0の場合のみキャッシュ有効

**LRUキャッシュ実装**:

```
struct InferenceCacheEntry {
    prompt_hash: u64,
    model_id: String,
    result: Vec<Token>,
    created_at: Instant,
    size_bytes: usize,
}
```

**サイズ管理**:

- 利用可能RAMの一定割合（デフォルト5%）を上限
- LRUポリシーでエビクション
- 再起動で全消失（永続化なし）

### リトライ設計

**指数バックオフパラメータ（固定値）**:

| パラメータ | 値 |
|-----------|-----|
| 初期待機時間 | 100ms |
| 最大リトライ回数 | 4回 |
| 最大待機時間 | 30秒 |
| 倍率 | 2倍 |

**リトライシーケンス**:

1. 失敗 → 100ms待機 → リトライ1
2. 失敗 → 200ms待機 → リトライ2
3. 失敗 → 400ms待機 → リトライ3
4. 失敗 → 800ms待機 → リトライ4
5. 失敗 → エラー返却

### キャンセル処理設計

**キャンセルフラグ**:

- 各リクエストにキャンセルフラグを保持
- トークン生成ループで毎回チェック
- フラグがセットされたら即座にループ離脱

**バッチ分離**:

- キャンセルされたリクエストのKVキャッシュを即座に解放
- バッチから当該リクエストを除外
- 他リクエストは影響なく継続

### 並行ロード設計

**ロード調整**:

- 複数モデルの並行ロードを許可
- GPU単位でのVRAM使用量を追跡
- ロード前にVRAM空き確認

**プラグインインスタンス管理**:

- プラグインIDごとに単一インスタンス
- 1つのプラグインが複数モデルを内部管理
- モデルごとの分離はプラグイン内部で実装

---

## Session 2025-12-31 追加設計 Part 4

### パラメータ検証設計

**Node側検証**:

- サンプリングパラメータの範囲チェック
  - temperature: 0.0 ～ 2.0
  - top_p: 0.0 ～ 1.0
  - top_k: 1 ～ vocab_size
- 不正値は即座に400エラー

**空プロンプト検証**:

- プロンプトが空または空白のみの場合は400 Bad Request
- Router側で早期検証（Nodeまで到達させない）

### stop sequences設計

**検出ロジック**:

- Node/プラグイン内の生成ループで実装
- 生成済みトークン列の末尾をチェック
- 複数のstop sequenceを同時監視可能

**実装方式**:

```
for each generated_token:
    append to output
    for each stop_seq in stop_sequences:
        if output.ends_with(stop_seq):
            truncate stop_seq from output
            return output
```

### logprobs設計

**返却フォーマット**:

- OpenAI互換のlogprobs構造を返却
- top_logprobs パラメータで上位N件を指定

**データ構造**:

```
{
  "logprobs": {
    "tokens": ["Hello", " world"],
    "token_logprobs": [-0.5, -0.3],
    "top_logprobs": [
      {"Hello": -0.5, "Hi": -1.2},
      {" world": -0.3, " there": -0.8}
    ]
  }
}
```

### max_tokensデフォルト値設計

**取得方法**:

1. モデルのconfig.jsonからmax_position_embeddingsを取得
2. プロンプト長を差し引いた残りを生成可能トークン数とする
3. 明示的なmax_tokens指定がある場合はそちらを優先

### アーキテクチャ検証設計

**チェックタイミング**:

1. モデルロードリクエスト受信時
2. manifestのarchitectures配列を取得
3. config.jsonのmodel_type/architecturesと照合
4. 不一致時はロード開始前にエラー返却

**エラーメッセージ**:

```
Model architecture 'llama' is not supported by plugin 'nemotron_engine'.
Supported: ['nemotron', 'mamba']
```

### フォーマット統合設計

**プラグインの責務**:

- 単一プラグインが複数フォーマット（GGUF/safetensors）をサポート可能
- ロード時にファイル拡張子/マジックバイトで判定
- 内部で適切なローダーに振り分け

**manifest.json拡張**:

```json
{
  "id": "llama_cpp",
  "formats": ["gguf", "safetensors"],
  "architectures": ["llama", "mistral", "gemma"]
}
```

## Session 2025-12-31 追加設計 Part 5

### 量子化指定設計

**モデル名フォーマット**:

- `modelname:quantization` 形式でAPI呼び出し時に指定
- 例: `llama-7b:Q4_K_M`, `mistral-7b:Q5_K_S`
- 量子化未指定時はデフォルト量子化を使用（登録時に設定）

**マッチングルール**:

- 完全一致のみ（正規化なし）
- 大文字/小文字区別あり
- ハイフン/アンダースコア区別あり
- 例: `Q4_K_M` ≠ `q4-k-m` ≠ `Q4-K-M`

### Prefix Caching設計

**キャッシュ戦略**:

- 同一システムプロンプトのKVキャッシュをリクエスト間で共有
- プロンプトハッシュをキーとしてKV状態を保持
- 共通プレフィックス部分のみキャッシュ

**メモリ管理**:

- モデルロード後の空きVRAMのN%（設定可能）をキャッシュに割当
- LRUでキャッシュエントリを管理
- VRAM圧迫時は古いエントリから削除

### mmproj自動検出設計

**検出ロジック**:

1. モデルディレクトリをスキャン
2. `mmproj-*.gguf` または `*-mmproj.gguf` パターンにマッチするファイルを検索
3. 見つかった場合は自動的にロード
4. 見つからない場合はテキストモデルとして動作

### レプリカ配置設計

**配置戦略**:

- 同一モデルを複数GPUに独立してロード
- 各レプリカは独立したKVキャッシュを持つ
- ホストが全レプリカのステータスを管理

**負荷分散**:

- ラウンドロビン方式でリクエストを振り分け
- レプリカ障害時は自動的にスキップ
- 健全なレプリカのみに振り分け

### chat_templateレンダリング設計

**Jinjaライブラリ**:

- inja（C++ヘッダーオンリーライブラリ）を使用
- HuggingFace互換のchat_templateをサポート
- カスタム関数（raise_exception等）は必要に応じて拡張

**レンダリングフロー**:

1. config.jsonからchat_templateを取得
2. messagesをJinjaコンテキストに変換
3. injaでテンプレートをレンダリング
4. 生成されたプロンプト文字列をプラグインに渡す

### Function Calling設計

**実装レイヤー**:

- Node/プラグイン側で実装
- tool定義をプロンプトに埋め込み
- 出力JSONパースでツール呼び出しを検出

**フロー**:

1. ツール定義をシステムプロンプトに追加
2. モデル出力からJSON形式のツール呼び出しを検出
3. 検出時はfinish_reason="tool_calls"で返却

### manifest.json拡張（Part 5）

```json
{
  "id": "llama_cpp",
  "formats": ["gguf", "safetensors"],
  "architectures": ["llama", "mistral", "gemma"],
  "modalities": ["completion", "embedding"],
  "license": "MIT",
  "supports_vision": true
}
```
