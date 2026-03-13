# Lessons Learned

ユーザーからの修正指示や作業中に学んだ教訓を記録し、再発を防止する。
セッション開始時にこのファイルを確認し、過去の教訓を踏まえて作業すること。

## 記録ルール

- 修正を受けたら、原因・正しい対応・再発防止ルールの3点を記録する
- 同じカテゴリの教訓はまとめて更新する
- 解消済み・陳腐化した教訓は削除する

## 教訓一覧

### tokio RwLock の write guard を .await をまたいで保持しない

- **事象**: `check_and_maybe_download` で `state.write().await` の write guard を名前付き変数として保持したまま `ensure_payload_ready().await` を呼び出し、内部で `state.read().await` を試みてデッドロック
- **原因**: tokio の `RwLock` はリエントラントではないため、同一タスクが write lock を保持したまま read lock を取得しようとすると永久にブロックされる。Rust の NLL はボローチェッカーの分析にのみ影響し、Drop のタイミング（スコープ末尾）は変えない
- **再発防止ルール**: `RwLockWriteGuard` / `RwLockReadGuard` は必ずスコープブロック `{ }` で囲み、`.await` をまたがせない。名前付きガード変数がある場合は `.await` の前に `drop()` するかブロックで囲む
- **次回チェック方法**: `state.write().await` を名前付き変数に束縛している箇所で、同一スコープ内に `.await` がないか grep で確認
