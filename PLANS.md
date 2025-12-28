# エンジン実装完了までの道筋（Node）

このドキュメントは、Node側の推論エンジン実装を「完了」と判断できる状態までの道筋をまとめたものです。
細分化ではなく、**大きな段階（フェーズ）**での到達点と検証観点を示します。

## 前提・原則
- 形式選択は**登録時に確定**（safetensors/GGUFの自動判別・実行時フォールバックは禁止）。
- `metadata.json` など独自メタデータには依存しない。
- NodeはPython依存を導入しない。
- エンジンは**動的プラグイン**として追加可能にする。
- Nemotron推論エンジンの詳細設計は**後回し（TBD）**。

## フェーズ0: 仕様の整合（完了条件の明確化）
**到達点**
- エンジン領域の境界が明確で、仕様間の矛盾がない。
- 内蔵エンジンの位置づけ・形式選択ルール・GPU前提が統一されている。

**検証**
- `specs/SPEC-3fc2c1e4/spec.md` と `specs/SPEC-d7feaa2c/spec.md` の整合性が取れている。
- `specs/SPEC-d7feaa2c/tasks.md` が最新の実装状況を反映している。

## フェーズ1: エンジン・プラグイン基盤の完成
**到達点**
- EngineHost がプラグインの**manifest検証**を行い、ABI互換を強制できる。
- 共有ライブラリの探索・ロード・アンロードの基盤が確立している。

**検証**
- EngineHostのユニットテストがGREEN。
- 不正なmanifest/ABI不一致は明確なエラーで拒否される。

## フェーズ2: プラグイン仕様の確定と実装
**到達点**
- Engine Pluginの **manifest.json** 仕様（必須項目・互換性条件）が確定。
- EngineHostがmanifest.jsonを読み取り、EngineRegistryに正しく登録できる。

**検証**
- manifestのJSONスキーマ/バリデーションが動作する。
- 実際のプラグイン（最小実装）をロードできる。

## フェーズ3: 既存エンジンのプラグイン化
**到達点**
- llama.cpp をプラグインとしてロード可能。
- 既存の推論フロー（chat/completions/embeddings）が維持される。

**検証**
- Nodeの既存テストが破綻しない。
- `llama_cpp` runtimeでの推論が既存と同等に動作する。

## フェーズ4: safetensors系エンジンの統合準備
**到達点**
- safetensors向けエンジンの枠組み（runtime・capabilities・選択ルール）が確定。
- gpt-oss など既存safetensors系の取り込みが可能。

**検証**
- format=safetensors の登録モデルが EngineRegistry を通じて解決される。
- エンジン未対応モデルは /v1/models から除外される。

## フェーズ5: 完了判定
**完了条件（最小）**
- プラグイン基盤（EngineHost + manifest検証 + 動的ロード）が実装済み。
- 主要エンジン（少なくとも llama.cpp / gpt-oss）が**プラグインとして動作**。
- 形式選択ルール（登録時確定）が守られ、実行時フォールバックが発生しない。
- テストはTDD方針に従ってRED→GREENの履歴が揃っている。

## 参考SPEC
- `specs/SPEC-3fc2c1e4/spec.md`（実行エンジン統合）
- `specs/SPEC-d7feaa2c/spec.md`（Nodeエンジンローダー抽象化）
- `specs/SPEC-2c0e5a9b/spec.md`（gpt-oss safetensors実行）
