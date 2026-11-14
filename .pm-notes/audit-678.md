# audit-678 結果要約 (Dashboard i18n) 【重要】
- i18n は 2026-06-29 に一度完全実装 → 同日ユーザー指示で 77c56e47 により全面撤去済み (develop/main 両方)
- Issue 本文は撤去前のまま乖離。i18n 基盤は現在ゼロ。UI/UX 改善分 (EmptyState/Skeleton/a11y) のみ維持されている
- 復活するなら git revert 77c56e47 が最短 (79ファイル +1085/-3223)。バッチ再実装は不合理
- 推奨: ユーザー判断必須 → (a) 経緯記録して close / (b) UI/UX のみに rescope / (c) revert 方針で本文書き換え+plan整備
- リスク: ユーザーが一度撤去指示した機能の無断復活は厳禁
