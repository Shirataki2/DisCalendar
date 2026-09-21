-- 一覧取得では未生成の区間だけを補充する。
ALTER TABLE event_series ADD COLUMN generated_from TIMESTAMP;
