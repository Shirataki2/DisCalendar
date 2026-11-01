-- Bot がサーバーに参加したときに送る最初の案内 (#376) の送信記録。
-- 退出直後の再参加 (追加し直し・誤って外した場合など) で何度も案内を送らないよう、
-- 最後に送った日時をサーバーごとに 1 行だけ持つ。
-- guilds と違って Bot の退出では消さない (消すと再参加のたびに送ってしまう)。
-- 書くのは bot だけで、api は読み書きしない
CREATE TABLE guild_welcome_messages (
    guild_id TEXT PRIMARY KEY,
    -- 最後に案内を送った日時 (タイムゾーンなしの JST。他のテーブルと同じ)
    sent_at TIMESTAMP NOT NULL
);
