-- #430 の support_release_announcements (更新告知の記録) が入った版から、その前の版 (v3.30.0 以前) へ戻すとき用。
--
-- **これは `migrations/` ではないので自動では実行されない。** 手で流すためのファイル。
--
-- 古い support-bot はこのテーブルを読まないが、`sqlx::migrate!` は既定 (ignore_missing = false) で
-- `_sqlx_migrations` に自分の知らないバージョンがあると起動に失敗するので、その記録だけを消す。
-- **テーブルは消さない** (告知済みの版を覚えておくため。もう一度新しい版へ進めたとき、
-- マイグレーションは CREATE TABLE IF NOT EXISTS なので当たり直っても記録は残り、同じ版を二度告知しない)。
--
-- 手順 (本番の compose。docs/operations.md「マイグレーションが入った版から戻す」と同じ要領):
--
--   1. support-bot を止める: docker compose stop support-bot
--   2. このファイルを流す:
--      docker compose exec -T db psql -U discalendar -d discalendar_support -v ON_ERROR_STOP=1 -1 < この.sql
--   3. 前の版のイメージでデプロイし直す (Actions の "Deploy production" に前のタグ)

DELETE FROM _sqlx_migrations WHERE version = 20261127030215;
