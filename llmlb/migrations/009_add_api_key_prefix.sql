-- APIキーにkey_prefixカラムを追加
-- UIで作成したキーと一覧のキーを照合できるようにする
ALTER TABLE api_keys ADD COLUMN key_prefix TEXT;
