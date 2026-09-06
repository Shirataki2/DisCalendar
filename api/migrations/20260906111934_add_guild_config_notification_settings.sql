-- guild_config に通知の設定を 2 つ追加する (#181)。web のサーバー設定ダイアログが書き、Bot と web が読む
-- (restricted と同じ役割分担)。通知先チャンネルは従来どおり event_settings (/init と共有) に置く。
--
-- - notify_at_start: 予定の開始時刻に通知するか。false なら Bot は「0 分前」を自動で足さない
--   (予定に保存された事前通知だけを送る)。既定は true = これまでの挙動
-- - default_notifications: web で予定を新規作成するときの事前通知の初期値。形式は events.notifications と
--   同じ JSONB (`[{ "num": 30, "unit": "minutes" }]`、api / bot の Notification::decode_all で読む)。
--   既定はこれまで web のフォームが持っていた初期値 (1 日前と 1 時間前) にして、設定しないサーバーの
--   使い勝手を変えない (api の models::guilds::DEFAULT_NOTIFICATIONS と同じ値)
ALTER TABLE guild_config
    ADD COLUMN notify_at_start BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN default_notifications JSONB NOT NULL
        DEFAULT '[{"num": 1, "unit": "days"}, {"num": 1, "unit": "hours"}]'::jsonb
        CHECK (jsonb_typeof(default_notifications) = 'array');
