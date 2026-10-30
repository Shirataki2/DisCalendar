-- #355: 毎月の繰り返しで日付・第n曜日を複数指定できるようにするため、条件 JSON を配列の形式にそろえる。
-- 旧: {"frequency":"monthly_date","day":5,...} / {"frequency":"monthly_weekday","nth":1,"weekday":6,...}
-- 新: {"frequency":"monthly_date","days":[5],...} / {"frequency":"monthly_weekday","nths":[1],"weekdays":[6],...}
-- api / bot は旧形式も読めるが、一覧や Webhook には保存した JSON をそのまま返すので形を統一する。
-- 戻し方は api/rollback/20261026000000_revert_multi_monthly_recurrence.sql。
UPDATE event_series
SET recurrence = (recurrence - 'day')
    || jsonb_build_object('days', jsonb_build_array(recurrence->'day'))
WHERE recurrence->>'frequency' = 'monthly_date' AND recurrence ? 'day';

UPDATE event_series
SET recurrence = (recurrence - 'nth' - 'weekday')
    || jsonb_build_object(
        'nths', jsonb_build_array(recurrence->'nth'),
        'weekdays', jsonb_build_array(recurrence->'weekday')
    )
WHERE recurrence->>'frequency' = 'monthly_weekday' AND recurrence ? 'nth';
