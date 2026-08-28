-- 利用者の日次アクティビティ (#81)。api がセッションを検証したリクエストを「その利用者がその日 (JST) に
-- 使った」として 1 人 1 日 1 行で積み上げ、正確な DAU / WAU / MAU を出す
-- (分析情報 #79 はセッションの生存期間からの推定で、この記録が無いことが限界だった)。
-- 記録するのは利用者 ID と日付の組だけで、IP アドレス・ユーザーエージェント・操作の内容は持たない
-- (プライバシーポリシーにもそのように記載する)。
-- 新規テーブルなので旧実装 / 旧 Bot とは共有しない
CREATE TABLE user_daily_activity (
    -- Better Auth の user.id
    user_id TEXT NOT NULL,
    -- 使った日 (JST の日付。api / bot の慣習どおり日付の区切りは JST)
    day DATE NOT NULL,
    PRIMARY KEY (user_id, day)
);

-- 集計 (期間で絞って人数を数える) 用
CREATE INDEX idx_user_daily_activity_day ON user_daily_activity (day);

-- 利用者に関する情報を削除するとき (退会の請求など) に記録も一緒に消えるよう外部キーを張る。
-- "user" は Better Auth (web 側) が作るテーブルで api のマイグレーションには含まれず、
-- まっさらな DB (テスト・新規環境。compose では api が web より先に起動する) にはまだ無いので、
-- あるときだけ張る。無いときは api が起動のたびに確かめて、"user" ができた後の起動で張り直す
-- (models::user_activity::ensure_user_fk)。それまでの差は「user の行を消したときに記録が
-- 残るかどうか」だけで、読み書きの挙動は変わらない
DO $$
BEGIN
    IF to_regclass('public."user"') IS NOT NULL THEN
        ALTER TABLE user_daily_activity
            ADD CONSTRAINT user_daily_activity_user_id_fkey
            FOREIGN KEY (user_id) REFERENCES "user" (id) ON DELETE CASCADE;
    END IF;
END
$$;
