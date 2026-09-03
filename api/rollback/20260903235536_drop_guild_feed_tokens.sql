-- #95 で追加した guild_feed_tokens を落とす (ロールバック用)。
--
-- **これは `migrations/` ではないので自動では実行されない。** 手で流すためのファイル。
--
-- 使うのは「#95 が入った版から、その前の版のイメージへ戻す」ときだけ。
-- 古い api はこのテーブルを読み書きしないので残しても動くが、`sqlx::migrate!` は既定
-- (ignore_missing = false) で `_sqlx_migrations` に自分の知らないバージョンがあると起動に
-- 失敗するため、テーブルと記録の両方を戻す。
--
-- 落とすと発行済みのフィード URL はすべて無効になる (外部カレンダー側の購読は取得エラーになる)。
-- 再びロールフォワードしてもトークンは戻らないので、利用者はサーバー設定から発行し直して
-- 外部カレンダーに登録し直す必要がある。必要なら先にダンプを取る (README「DB のバックアップと復元」)。
--
-- 手順 (compose の環境。README「マイグレーションが入った版から戻す」と同じ要領):
--
--   1. api を止める:       docker compose stop api
--   2. 先にダンプを取る:   docker compose exec -T db pg_dump -U discalendar -d discalendar -Fc > 戻す前.dump
--   3. このファイルを流す: docker compose exec -T db psql -U discalendar -d discalendar -v ON_ERROR_STOP=1 -1 < この.sql
--   4. 前の版のイメージでデプロイし直す (Actions の "Deploy production" に前のタグ)

DROP TABLE guild_feed_tokens;

-- 古い api の migrations/ に無いバージョンを消す (残っていると起動時に VersionMissing で落ちる)。
-- 戻したあと再びロールフォワードすれば、このマイグレーションが改めて適用される
DELETE FROM _sqlx_migrations WHERE version = 20260903235536;
