-- 補充済み範囲に隙間を抱えたシリーズがあるため (#362)、記録を一度消して次の一覧取得・日次補充で補充し直す。
-- 生成済みの回は (series_id, original_start_at) の一意制約で重複しない。予定の行は変更しない。
UPDATE event_series SET generated_from = NULL, generated_until = NULL;
