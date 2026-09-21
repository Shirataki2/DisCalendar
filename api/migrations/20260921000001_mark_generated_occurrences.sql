-- 作成数の推移から補充した回を除外する。単発へ切り離した後も生成元を保持する。
ALTER TABLE events ADD COLUMN generated_from_series BOOLEAN NOT NULL DEFAULT false;
UPDATE events e SET generated_from_series = true
FROM event_series s WHERE e.series_id=s.id AND e.original_start_at<>s.start_at;
