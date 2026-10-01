-- SPEC #777 FR-001: 運用通知の宛先となるメールアドレス
--
-- email は通知先であり、ログイン識別子ではない（ログインは username のまま）。
-- 既存ユーザーは NULL のまま全機能を使える。UNIQUE 制約は置かない
-- （複数のユーザーが同じ通知先を共有できる）。
ALTER TABLE users ADD COLUMN email TEXT;
