-- #355 で配列の形式にした毎月の繰り返し条件を、旧形式 (day / nth + weekday) に戻す (ロールバック用)。
-- `migrations/` ではないので自動では実行されない。#355 が入った版から前の版へ戻すときだけ手で流す。
--
-- 古い api / bot は旧形式しか読めないので、戻さずに起動すると毎月のシリーズの一覧・補充・通知が失敗する。
--
-- 日付や第n曜日を複数指定したシリーズは旧形式で表せない。条件の変更・解除・「この回以降」の分割をしても
-- 元のシリーズ行は終了済みとして残るので、一度でも複数指定を使っていれば必ず該当する。そのため #90 の戻し方と
-- 同じく、複数指定のシリーズは次のように扱う:
--   - シリーズと例外を event_series_multi_monthly_backup / event_series_exceptions_multi_monthly_backup に退避する
--   - 生成済みの各回は ID・添付・共有リンクを保ったまま単発の予定として切り離す (中止した回はもともと行が無い)
--   - シリーズ行を削除する。未生成の将来分は補充されなくなり、作成数の集計からもそのシリーズの分が外れる
-- 退避テーブルが既にあれば上書きせずに停止するので、先に中身を確認してから名前を変えるか削除する。
--
-- 手順: api / bot を停止し、バックアップを取得してから
--   docker compose exec -T db psql -U discalendar -d discalendar -v ON_ERROR_STOP=1 -1 < この.sql
-- を実行し、前の版のイメージでデプロイし直す (docs/operations.md「マイグレーションが入った版から戻す」)。
BEGIN;
LOCK TABLE event_series, event_series_exceptions, events IN SHARE ROW EXCLUSIVE MODE;

CREATE TABLE event_series_multi_monthly_backup AS
SELECT *
FROM event_series
WHERE (recurrence->>'frequency' = 'monthly_date'
       AND jsonb_array_length(recurrence->'days') <> 1)
   OR (recurrence->>'frequency' = 'monthly_weekday'
       AND (jsonb_array_length(recurrence->'nths') <> 1
            OR jsonb_array_length(recurrence->'weekdays') <> 1));

CREATE TABLE event_series_exceptions_multi_monthly_backup AS
SELECT x.*
FROM event_series_exceptions x
WHERE x.series_id IN (SELECT id FROM event_series_multi_monthly_backup);

UPDATE events
SET series_id = NULL, original_start_at = NULL
WHERE series_id IN (SELECT id FROM event_series_multi_monthly_backup);

-- 例外は ON DELETE CASCADE で一緒に消える (退避済み)。
DELETE FROM event_series
WHERE id IN (SELECT id FROM event_series_multi_monthly_backup);

UPDATE event_series
SET recurrence = (recurrence - 'days')
    || jsonb_build_object('day', recurrence->'days'->0)
WHERE recurrence->>'frequency' = 'monthly_date' AND recurrence ? 'days';

UPDATE event_series
SET recurrence = (recurrence - 'nths' - 'weekdays')
    || jsonb_build_object(
        'nth', recurrence->'nths'->0,
        'weekday', recurrence->'weekdays'->0
    )
WHERE recurrence->>'frequency' = 'monthly_weekday' AND recurrence ? 'nths';

DELETE FROM _sqlx_migrations WHERE version = 20261026000000;
COMMIT;
