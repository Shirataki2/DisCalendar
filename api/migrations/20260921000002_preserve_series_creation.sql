-- 初回の中止・切り離しに影響されない作成記録。分割先は作成数に加えない。
ALTER TABLE event_series ADD COLUMN is_creation BOOLEAN NOT NULL DEFAULT true;
-- このPRの旧スキーマで作った分割は同じ作成者・作成日時を引き継ぐ。
UPDATE event_series s SET is_creation=false
WHERE EXISTS (SELECT 1 FROM event_series root WHERE root.guild_id=s.guild_id
    AND root.created_by=s.created_by AND root.created_at=s.created_at AND root.id<s.id);
UPDATE events SET generated_from_series=true WHERE series_id IS NOT NULL;
CREATE VIEW event_creations AS
    SELECT guild_id,created_at FROM events WHERE NOT generated_from_series
    UNION ALL
    SELECT guild_id,created_at FROM event_series WHERE is_creation;
