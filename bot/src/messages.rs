//! Bot の固定案内・Discord 登録用の翻訳。未定義の文は日本語へ戻す。
use crate::i18n::Locale;
pub fn message(locale: Locale, ja: &str) -> &str {
    if locale == Locale::Ja {
        return ja;
    }
    match ja {
        "このコマンドはサーバー内でのみ実行できます" => {
            "This command can only be used in a server."
        }
        "予定の名称 (32 文字まで)" => "Event title (up to 32 characters)",
        "予定の説明 (1000 文字まで)" => "Event description (up to 1000 characters)",
        "場所または URL (200 文字まで)" => "Location or URL (up to 200 characters)",
        "終日の予定にする (時・分は無視されます)" => {
            "All-day event (hours and minutes are ignored)"
        }
        "予定の色 (省略時は青)" => "Event color (blue by default)",
        "日本時間の日付: 今日・明日・YYYY-MM-DD" => {
            "Date in JST: today, tomorrow, or YYYY-MM-DD"
        }
        "日本時間の開始時刻: HH:mm (例: 21:00)" => {
            "Start time in JST: HH:mm (e.g. 21:00)"
        }
        "所要時間 (分)。省略時は 60 分、1〜10080 分" => {
            "Duration in minutes (1 to 10080; default: 60)"
        }
        "通知先のチャンネル (指定しない場合はこのコマンドを実行したチャンネル)" => {
            "Notification channel (defaults to the current channel)"
        }
        "表示する予定の範囲 (省略時は未来)" => "Event range (future by default)",
        "開始" => "Start",
        "終了" => "End",
        "場所" => "Location",
        "通知" => "Notifications",
        "予定を作成しました" => "Event created.",
        "作成する" => "Create",
        "キャンセル" => "Cancel",
        "保存しています…" => "Saving…",
        "作成を取り消しました。もう一度コマンドを実行して入力できます" => {
            "Creation canceled. Run the command again to enter a new event."
        }
        "作成処理が完了しました" => "Event creation completed.",
        "作成処理を完了できませんでした。エラーの案内を確認してください" => {
            "Could not create the event. Please check the error message."
        }
        "予定一覧" => "Events",
        "繰り返し予定は過去366日〜未来730日を表示します" => {
            "Recurring events are shown from 366 days ago to 730 days ahead."
        }
        "なし" => "None",
        "過去" => "Past",
        "未来" => "Future",
        "全て" => "All",
        "今日" => "Today",
        "今週" => "This week",
        "次の予定" => "Next event",
        "今日の予定はありません" => "No events today.",
        "今週の予定はありません" => "No events this week.",
        "これから開催される予定はありません" => "No upcoming events.",
        "過去の予定はありません" => "No past events.",
        "登録されている予定はありません" => "No events have been added.",
        "保存されたチャンネル ID が不正です" => "The saved channel ID is invalid.",
        "未設定" => "Not configured",
        "取得失敗（未設定かどうかも確認できません）" => {
            "Unavailable (could not determine whether a channel is configured)"
        }
        "有効" => "Enabled",
        "無効" => "Disabled",
        "あり" => "Yes",
        "開始時刻の通知: 取得失敗\n既定の事前通知: 取得失敗\n時間をおいて `/settings` を再実行してください。" => {
            "Start notifications: unavailable\nDefault reminders: unavailable\nPlease try `/settings` again later."
        }
        "通知先が未設定です。`/init` または Web のサーバー設定で通知先を設定してください。" => {
            "No notification channel is configured. Use `/init` or the server settings on the website."
        }
        "保存された通知先をこのサーバーのチャンネルとして確認できません。`/init` または Web のサーバー設定で再設定してください。" => {
            "Could not verify that the saved channel belongs to this server. Set it again with `/init` or the server settings on the website."
        }
        "Discord が「不明なチャンネル」を返しました。通知先は削除された可能性があります。`/init` または Web のサーバー設定で通知先を再設定してください。" => {
            "Discord reported an unknown channel. It may have been deleted. Set it again with `/init` or the server settings on the website."
        }
        "Bot から通知先を参照できません。チャンネルの存在と Bot のロール・チャンネルの権限設定を確認し、必要なら `/init` で通知先を再設定してください。投稿権限は確認できていません。" => {
            "The bot cannot access the notification channel. Check that it exists and review the bot role and channel permissions. Use `/init` to reset it if needed. Posting permissions could not be checked."
        }
        "取得失敗のため投稿権限を確認できません。時間をおいて `/settings` を再実行してください。" => {
            "Could not retrieve posting permissions. Please try `/settings` again later."
        }
        "投稿に必要な権限は揃っています。" => {
            "The bot has the permissions required to post."
        }
        "チャンネルを見る" => "View Channel",
        "メッセージを送信" => "Send Messages",
        "スレッドでメッセージを送信" => "Send Messages in Threads",
        "埋め込みリンク" => "Embed Links",
        "このコマンドを実行するには「管理者」「サーバー管理」「メッセージの管理」「ロールの管理」のいずれかの権限が必要です" => {
            "You need Administrator, Manage Server, Manage Messages, or Manage Roles to use this command."
        }
        "予定の作成には管理権限、または編集を許可されたロールが必要です" => {
            "Creating events requires management permissions or an allowed editor role."
        }
        "予定にする" => "Create event",
        "前へ" => "Previous",
        "完了" => "Done",
        "次へ" => "Next",
        "このBotの使い方を表示します" => "Show how to use this bot",
        "予定を新たに作成します" => "Create an event",
        "少ない入力で予定を作成します（保存前に日時を確認）" => {
            "Create an event with fewer inputs and confirm its date before saving"
        }
        "予定の一覧を表示します" => "List events",
        "このBotの通知の送信先チャンネルを設定します" => {
            "Set the notification channel for this bot"
        }
        "現在の通知設定とBotの投稿権限を確認します（本人にだけ表示）" => {
            "Check notification settings and bot permissions (only visible to you)"
        }
        "Botを他のサーバーに招待するためのURLを表示します" => {
            "Show a link to invite the bot to another server"
        }
        "予定にする（日本時間 / JST）" => "Create event (Japan time / JST)",
        "タイトル" => "Title",
        "日付（YYYY-MM-DD・今日・明日）" => "Date (YYYY-MM-DD, today, tomorrow)",
        "例: 2026-10-03" => "e.g. 2026-10-03",
        "開始時刻（HH:mm）" => "Start time (HH:mm)",
        "例: 21:00" => "e.g. 21:00",
        "所要時間（分・1〜10080）" => "Duration (minutes, 1 to 10080)",
        "例: 120" => "e.g. 120",
        "開始日時 (年)" => "Start date/time (year, JST)",
        "開始日時 (月)" => "Start date/time (month, JST)",
        "開始日時 (日)" => "Start date/time (day, JST)",
        "開始日時 (時)" => "Start date/time (hour, JST)",
        "開始日時 (分)" => "Start date/time (minute, JST)",
        "終了日時 (年)" => "End date/time (year, JST)",
        "終了日時 (月)" => "End date/time (month, JST)",
        "終了日時 (日)" => "End date/time (day, JST)",
        "終了日時 (時)" => "End date/time (hour, JST)",
        "終了日時 (分)" => "End date/time (minute, JST)",
        "事前通知 (1 つ目)" => "Reminder 1",
        "事前通知 (2 つ目)" => "Reminder 2",
        "事前通知 (3 つ目)" => "Reminder 3",
        "事前通知 (4 つ目)" => "Reminder 4",
        "赤" => "Red",
        "青" => "Blue",
        "緑" => "Green",
        "黄" => "Yellow",
        "紫" => "Purple",
        "水色" => "Light blue",
        "橙" => "Orange",
        "ピンク" => "Pink",
        "灰" => "Gray",
        "黒" => "Black",
        "5分前" => "5 minutes before",
        "10分前" => "10 minutes before",
        "15分前" => "15 minutes before",
        "30分前" => "30 minutes before",
        "1時間前" => "1 hour before",
        "2時間前" => "2 hours before",
        "3時間前" => "3 hours before",
        "6時間前" => "6 hours before",
        "12時間前" => "12 hours before",
        "1日前" => "1 day before",
        "2日前" => "2 days before",
        "3日前" => "3 days before",
        "7日前" => "7 days before",
        _ => ja,
    }
}
