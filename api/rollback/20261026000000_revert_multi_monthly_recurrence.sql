-- #355 で配列の形式にした毎月の繰り返し条件を、旧形式 (day / nth + weekday) に戻す (ロールバック用)。
-- `migrations/` ではないので自動では実行されない。#355 が入った版から前の版へ戻すときだけ手で流す。
--
-- 古い api / bot は旧形式しか読めないので、戻さずに起動すると毎月のシリーズの一覧・補充・通知が失敗する。
-- 日付や第n曜日を複数指定したシリーズは旧形式で表せない。そのようなシリーズがあれば何も変えずに停止するので、
-- 対応版のうちに Web で条件を 1 つにするか繰り返しを解除してから流し直す。
--
-- 手順: api / bot を停止し、バックアップを取得してから
--   docker compose exec -T db psql -U discalendar -d discalendar -v ON_ERROR_STOP=1 -1 < この.sql
-- を実行し、前の版のイメージでデプロイし直す (docs/operations.md「マイグレーションが入った版から戻す」)。
BEGIN;
LOCK TABLE event_series IN SHARE ROW EXCLUSIVE MODE;

DO $$
DECLARE
    multi INTEGER;
BEGIN
    SELECT count(*) INTO multi
    FROM event_series
    WHERE (recurrence->>'frequency' = 'monthly_date'
           AND jsonb_array_length(recurrence->'days') <> 1)
       OR (recurrence->>'frequency' = 'monthly_weekday'
           AND (jsonb_array_length(recurrence->'nths') <> 1
                OR jsonb_array_length(recurrence->'weekdays') <> 1));
    IF multi > 0 THEN
        RAISE EXCEPTION '日付・第n曜日を複数指定した毎月のシリーズが % 件あります。旧形式で表せないため、条件を 1 つにするか繰り返しを解除してから実行してください', multi;
    END IF;
END $$;

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
