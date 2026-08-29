-- guilds に参加日時 joined_at を追加する (#125)。
-- 日時は他のカラムと同じタイムゾーンなしの JST。bot が新規参加の INSERT で設定する。
-- 既存レコードの backfill (入退出時にログチャンネルへ送っていたメッセージのログ由来) は、
-- 本番ギルド ID を公開リポジトリに含めないため、このマイグレーションには入れない。
-- `api/scripts/backfill_guilds_joined_at.py` で CSV (git 管理外) から SQL を生成して手で流す
-- (手順はスクリプト冒頭のコメント)。backfill されないギルドは NULL のまま残る。
ALTER TABLE guilds ADD COLUMN joined_at TIMESTAMP;
