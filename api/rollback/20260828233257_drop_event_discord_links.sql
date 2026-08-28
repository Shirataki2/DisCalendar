-- #94 で追加した event_discord_links を落とす (ロールバック用)。
--
-- **これは `migrations/` ではないので自動では実行されない。** 手で流すためのファイル。
--
-- 使うのは「#94 が入った版から、その前の版のイメージへ戻す」ときだけ。
-- 古い api はこのテーブルを読み書きしないので残しても動くが、`sqlx::migrate!` は既定
-- (ignore_missing = false) で `_sqlx_migrations` に自分の知らないバージョンがあると起動に
-- 失敗するため、テーブルと記録の両方を戻す。
--
-- 対応付けを消しても Discord 側のスケジュールイベント自体は消えない。予定との紐付けが
-- 失われるだけなので、不要になったイベントは Discord 側で手動で削除する。
-- 再びロールフォワードしても対応付けは戻らない (連携し直すには予定を編集してオプションを
-- 入れ直す)。必要なら先にダンプを取る (README「DB のバックアップと復元」)。
--
-- 手順 (compose の環境。README「マイグレーションが入った版から戻す」と同じ要領):
--
--   1. api を止める:       docker compose stop api
--   2. 先にダンプを取る:   docker compose exec -T db pg_dump -U discalendar -d discalendar -Fc > 戻す前.dump
--   3. このファイルを流す: docker compose exec -T db psql -U discalendar -d discalendar -v ON_ERROR_STOP=1 -1 < この.sql
--   4. 前の版のイメージでデプロイし直す (Actions の "Deploy production" に前のタグ)

DROP TABLE event_discord_links;

-- 古い api の migrations/ に無いバージョンを消す (残っていると起動時に VersionMissing で落ちる)。
-- 戻したあと再びロールフォワードすれば、このマイグレーションが改めて適用される
DELETE FROM _sqlx_migrations WHERE version = 20260828233257;
