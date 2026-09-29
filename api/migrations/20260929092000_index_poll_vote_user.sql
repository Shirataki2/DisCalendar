-- 参加者名の照会時にユーザーIDから回答を引けるようにする。
CREATE INDEX schedule_poll_votes_user ON schedule_poll_votes(user_id, option_id);
