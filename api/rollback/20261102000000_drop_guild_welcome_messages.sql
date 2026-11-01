-- #376 で追加した guild_welcome_messages を落とす (ロールバック用)。
--
-- **これは `migrations/` ではないので自動では実行されない。** 手で流すためのファイル。
--
-- 使うのは「#376 が入った版から、その前の版のイメージへ戻す」ときだけ。
-- 古い api / bot はこのテーブルを読み書きしないので残しても動くが、`sqlx::migrate!` は既定
-- (ignore_missing = false) で `_sqlx_migrations` に自分の知らないバージョンがあると起動に
-- 失敗するため、テーブルと記録の両方を戻す。
--
-- 送信記録はこのファイルを流すと消える。再びロールフォワードした後は、30 日以内に案内を送った
-- サーバーにも再参加時にもう一度送ることがある (それ以外の影響はない)。
--
-- 手順 (compose の環境。docs/operations.md「マイグレーションが入った版から戻す」と同じ要領):
--
--   1. api / bot を止める: docker compose stop api bot
--   2. このファイルを流す: docker compose exec -T db psql -U discalendar -d discalendar -v ON_ERROR_STOP=1 -1 < この.sql
--   3. 前の版のイメージでデプロイし直す (Actions の "Deploy production" に前のタグ)

DROP TABLE guild_welcome_messages;

-- 古い api の migrations/ に無いバージョンを消す (残っていると起動時に VersionMissing で落ちる)。
-- 戻したあと再びロールフォワードすれば、このマイグレーションが改めて適用される
DELETE FROM _sqlx_migrations WHERE version = 20261102000000;
