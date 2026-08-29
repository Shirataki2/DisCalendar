-- #125 で追加した guilds.joined_at を落とす (ロールバック用)。
--
-- **これは `migrations/` ではないので自動では実行されない。** 手で流すためのファイル。
--
-- 使うのは「#125 が入った版から、その前の版のイメージへ戻す」ときだけ。
-- 古い api / bot はこのカラムを読み書きしないので残しても動くが、`sqlx::migrate!` は既定
-- (ignore_missing = false) で `_sqlx_migrations` に自分の知らないバージョンがあると起動に
-- 失敗するため、カラムと記録の両方を戻す。
--
-- ログ由来の backfill と bot が記録した参加日時はこのファイルを流すと消える。再びロール
-- フォワードしてもカラムが空で戻るだけなので、backfill は
-- `api/scripts/backfill_guilds_joined_at.py` で改めて流し直す。
--
-- 手順 (compose の環境。README「マイグレーションが入った版から戻す」と同じ要領)。
-- 現行 bot は参加・更新のたびに joined_at へ書き込むので、api だけでなく bot も止めてから
-- カラムを落とす (動かしたままだと旧イメージに置き換わるまで GuildCreate/GuildUpdate の
-- クエリが失敗し続ける):
--
--   1. api / bot を止める: docker compose stop api bot
--   2. 先にダンプを取る:   docker compose exec -T db pg_dump -U discalendar -d discalendar -Fc > 戻す前.dump
--   3. このファイルを流す: docker compose exec -T db psql -U discalendar -d discalendar -v ON_ERROR_STOP=1 -1 < この.sql
--   4. 前の版のイメージでデプロイし直す (Actions の "Deploy production" に前のタグ)

ALTER TABLE guilds DROP COLUMN joined_at;

-- 古い api の migrations/ に無いバージョンを消す (残っていると起動時に VersionMissing で落ちる)。
-- 戻したあと再びロールフォワードすれば、このマイグレーションが改めて適用される
DELETE FROM _sqlx_migrations WHERE version = 20260830030048;
