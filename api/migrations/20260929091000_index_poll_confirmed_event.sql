-- 予定削除時の外部キー更新で日程調整全体を走査しない。
CREATE INDEX schedule_polls_confirmed_event ON schedule_polls(confirmed_event_id) WHERE confirmed_event_id IS NOT NULL;
