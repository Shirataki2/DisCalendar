-- #403 で追加した event_attachments.discord_cover を落とす (ロールバック用)。
--
-- **これは `migrations/` ではないので自動では実行されない。** 手で流すためのファイル。
--
-- 使うのは「#403 が入った版から、その前の版のイメージへ戻す」ときだけ。
-- 古い api はこのカラムを読まないので残しても動くが、`sqlx::migrate!` は既定
-- (ignore_missing = false) で `_sqlx_migrations` に自分の知らないバージョンがあると起動に
-- 失敗するため、カラムと記録の両方を戻す。
--
-- どの添付をカバーにしていたかはこのファイルを流すと失われる。添付そのものと、
-- Discord 側に設定済みのカバー画像は残る (Discord 側の画像は、古い版では変更・解除できない)。
--
-- 手順 (compose の環境。docs/operations.md「マイグレーションが入った版から戻す」と同じ要領):
--
--   1. api / bot を止める: docker compose stop api bot
--   2. このファイルを流す: docker compose exec -T db psql -U discalendar -d discalendar -v ON_ERROR_STOP=1 -1 < この.sql
--   3. 前の版のイメージでデプロイし直す (Actions の "Deploy production" に前のタグ)

DROP INDEX event_attachments_one_discord_cover;
ALTER TABLE event_attachments
    DROP CONSTRAINT event_attachments_discord_cover_image,
    DROP COLUMN discord_cover;

-- 古い api の migrations/ に無いバージョンを消す (残っていると起動時に VersionMissing で落ちる)。
-- 戻したあと再びロールフォワードすれば、このマイグレーションが改めて適用される
DELETE FROM _sqlx_migrations WHERE version = 20261121000000;
