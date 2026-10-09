/** 日本語の原文をキーにする。英訳のない案内は原文へフォールバックする。 */
export const english: Readonly<
  Record<
    string,
    | string
    | {
        one: string;
        other: string;
      }
  >
> = {
  "サーバーの通知・投稿の言語": "Server notification and post language",
  "チャンネル通知・まとめ・日程調整の投稿に使います。個人への返信とプッシュ通知の言語は変わりません。":
    "Used for channel notifications, digests, and scheduling polls. Personal replies and push notifications use your own language.",
  なし: "None",
  予定を編集: "Edit event",
  予定を複製: "Duplicate event",
  予定を作成: "Create event",
  "保存先:": "Save to:",
  基本: "Basic",
  "繰り返し・通知": "Repeat & reminders",
  通知: "Reminders",
  "添付・共有": "Files & sharing",
  "この回以降を変更します。回数を変えなければ、消費済みの開催枠を差し引きます。個別編集済みの回は保持し、新しい条件から外れる個別編集済みの回・添付のある回は単発として残します。":
    "Changes apply to this and future occurrences. If the count stays the same, used occurrences are deducted. Individually edited occurrences are kept. Edited occurrences and those with attachments outside the new schedule remain as standalone events.",
  "この回のみを変更します。繰り返し条件は変更されません。":
    "Only this occurrence will change. The repeat schedule stays the same.",
  "元の予定の日時を引き継いでいます。保存前に確認してください。":
    "The dates and times were copied from the original event. Check them before saving.",
  終日: "All day",
  "（日本時間）": " (Japan time)",
  タイトル: "Title",
  タイトルを入力: "Enter a title",
  開始日: "Start date",
  開始時刻: "Start time",
  終了日: "End date",
  終了時刻: "End time",
  色: "Color",
  "場所 / URL": "Location / URL",
  説明: "Description",
  説明のプレビュー: "Description preview",
  "説明を入力すると、ここにプレビューが表示されます。":
    "Enter a description to see a preview here.",
  "**太字**・- リストなどの書式": "Formatting such as **bold** and - lists",
  プレビュー: "Preview",
  "Discordイベント連携を解除して保存すると、繰り返しを設定できます。":
    "Disable Discord event sync and save to enable repeating events.",
  "Discordイベント連携は後日対応です。添付ファイルは選択した回だけに保存します。":
    "Discord event sync for repeating events is coming later. Attachments are saved only to the selected occurrence.",
  事前通知: "Advance reminders",
  "予定は保存済みです。添付ファイルの送信を完了してください。":
    "The event is saved. Finish uploading the attachments.",
  削除: "Delete",
  キャンセル: "Cancel",
  "保存中…": "Saving…",
  保存: "Save",
  作成: "Create",
  "未保存の変更を破棄しますか？": "Discard unsaved changes?",
  "入力した未保存の変更と未送信の添付は失われます。 保存済みの予定と添付は残ります。":
    "Unsaved changes and pending uploads will be lost. Saved events and attachments will remain.",
  編集を続ける: "Keep editing",
  破棄して閉じる: "Discard and close",
  "（入力エラーあり）": "(has input errors)",
  "Discord のイベントとしても作成する": "Also create a Discord event",
  "確認中…": "Checking…",
  権限を再確認: "Recheck permissions",
  "Discord 側の権限はまだ変わっていません":
    "Discord permissions have not changed yet",
  "確認できませんでした。時間をおいて試してください":
    "Could not check permissions. Try again later",
  "開始日時が過去の予定は Discord のイベントにできません (連携済みの予定は保存すると連携が解除されます)":
    "Events starting in the past cannot be synced to Discord. Saving an already synced event will disable its sync.",
  "あなたに Discord の「イベントの作成」権限がないため、この連携を作り直すことはできません。チェックを外すと連携を解除します":
    "You need Discord's “Create Events” permission to recreate this sync. Uncheck this option to disable it.",
  "Discord の「イベントの作成」権限を持つ人だけが利用できます。サーバーの管理者にロールの権限を確認してください":
    "Discord's “Create Events” permission is required. Ask a server administrator to check your role permissions.",
  "Bot に「イベントの作成」権限がないため、変更は Discord に反映できません。チェックを外すと連携を解除します":
    "The Bot lacks Discord's “Create Events” permission, so changes cannot be synced. Uncheck this option to disable sync.",
  "予定の作成・変更・削除を Discord のスケジュールイベントにも反映します":
    "Creating, editing and deleting this event will also update its Discord scheduled event.",
  "Bot に「イベントの作成」権限がないため利用できません。":
    "The Bot needs Discord's “Create Events” permission. ",
  "Bot を招待し直す": "Reinvite the Bot",
  と利用できます: " to enable this feature",
  "他の人が繰り返し予定を変更しました。画面を閉じて予定を開き直し、最新の内容で確認してください。":
    "Someone changed the repeating event. Close this form and reopen the event to review its latest details.",
  入力内容を確認してください: "Check your input",
  予定をクイック追加: "Quick add event",
  "タイトルを入力して Enter": "Enter a title and press Enter",
  詳細を入力: "Add details",
  "作成中…": "Creating…",
  "新規作成 (n)": "New event (n)",
  このサーバーでは管理権限または指定ロールを持つメンバーが予定を編集できます:
    "Only members with management permissions or an assigned role can edit events in this server",
  新規作成: "New event",
  作成メニューを開く: "Open create menu",
  ICSファイルから取り込む: "Import an ICS file",
  予定を編集できません: "You cannot edit events",
  日程調整: "Scheduling polls",
  "読み込み中…": "Loading…",
  "予定を取得できませんでした:": "Could not load events:",
  再試行: "Retry",
  閉じる: "Close",
  外部カレンダーの凡例: "External calendar legend",
  取得できません: "Cannot load",
  一部省略: "Some events omitted",
  "外部カレンダーを取得できませんでした。":
    "Could not load external calendars.",
  "· 外部カレンダーの予定": "· External calendar event",
  "予定を削除しますか？": "Delete this event?",
  削除する予定: "Events to delete",
  この回のみ: "This occurrence",
  この回以降: "This and future occurrences",
  "元の開催日がこの回以降の予定を、個別編集済みの回と添付ファイルも含めて削除します。":
    "All occurrences originally scheduled from this date onward will be deleted, including edited occurrences and their attachments.",
  変更する予定: "Events to change",
  "変更を適用する範囲を選んでください。": "Choose which occurrences to change.",
  操作ガイド: "Guide",
  "（この回は個別変更）": " (this occurrence was edited)",
  このサーバーのカレンダーを開く: "Open this server's calendar",
  編集: "Edit",
  複製: "Duplicate",
  添付ファイル: "Attachments",
  添付ファイルは現在利用できません: "Attachments are currently unavailable",
  再読み込み: "Reload",
  添付ファイルはありません: "No attachments",
  ダウンロード: "Download",
  添付を削除: "Delete attachment",
  画像を再読み込み: "Reload image",
  添付は予定ごとに10件までです: "Up to 10 attachments per event",
  保存後に送信: "Upload after saving",
  "送信中…": "Uploading…",
  送信失敗: "Upload failed",
  添付の追加: "Add attachments",
  ファイルを添付: "Attach files",
  "JPEG・PNG・WebP・PDF / 1件 約10.5 MB、予定ごと10件まで":
    "JPEG, PNG, WebP or PDF / About 10.5 MB each, up to 10 files per event",
  容量制限とサーバー使用量: "Limits and server storage",
  "サーバー使用量（送信待ちを含む）:":
    "Server storage (including pending uploads):",
  "/ 約1.07 GB": "/ about 1.07 GB",
  選択を解除: "Remove selection",
  添付を再試行: "Retry uploads",
  記録なし: "Not recorded",
  変更履歴: "History",
  "この予定を誰がいつどう変えたかを新しい順に表示します。直近 50 件まで表示し、180 日を過ぎた履歴は消えます。":
    "Shows who changed this event, when, and how, newest first. Up to the latest 50 changes are shown, and history older than 180 days is removed.",
  変更履歴を読み込めませんでした: "Could not load the history",
  この予定の変更履歴はまだありません:
    "No changes have been recorded for this event yet",
  予定を作成しました: "created the event",
  予定を変更しました: "changed the event",
  予定を削除しました: "deleted the event",
  予定を復元しました: "restored the event",
  "Bot から": "via the bot",
  管理コンソールから: "via the admin console",
  "AI アシスタントから": "via an AI assistant",
  "DisCalendar の運営": "DisCalendar staff",
  "タイトル: 「{before}」→「{after}」": "Title: “{before}” → “{after}”",
  "日時: {before} → {after}": "Date & time: {before} → {after}",
  "場所: {before} → {after}": "Location: {before} → {after}",
  説明を変更: "Changed the description",
  繰り返しの設定を変更: "Changed the repeat settings",
  "色:": "Color:",
  "通知: {before} → {after}": "Reminders: {before} → {after}",
  通知のメンション先を変更: "Changed who is mentioned in reminders",
  "Discord のイベントと連携": "Linked to a Discord event",
  "Discord のイベントとの連携を解除": "Unlinked from the Discord event",
  メンバー情報を取得できません: "Could not load member details",
  退出したメンバー: "Member left the server",
  "作成:": "Created by:",
  "最終更新:": "Last updated by:",
  共有リンクを無効化しました: "Sharing link disabled",
  共有リンクをコピーしました: "Sharing link copied",
  "コピーできませんでした。下の URL を選択してコピーしてください":
    "Could not copy the link. Select and copy the URL below",
  予定の共有: "Share event",
  "リンクを知っている人は、ログインせずに保存済みの予定とサーバー名を閲覧できます。":
    "Anyone with the link can view the saved event and server name without signing in.",
  共有リンクをコピー: "Copy sharing link",
  共有リンクを無効化: "Disable sharing link",
  "共有リンク URL": "Sharing link URL",
  "← 予定に戻る": "← Back to event",
  繰り返しの設定: "Repeat settings",
  "条件と開催日を確認して、予定のフォームに適用します。":
    "Review the schedule and dates, then apply them to the event.",
  繰り返しの頻度: "Repeat frequency",
  繰り返しなし: "Does not repeat",
  毎日: "Daily",
  毎週: "Weekly",
  隔週: "Every 2 weeks",
  "毎月（日付）": "Monthly (by date)",
  "毎月（第n曜日）": "Monthly (by weekday)",
  "曜日（複数選択）": "Weekdays (select multiple)",
  毎月の日付: "Day of the month",
  週の順番: "Week of the month",
  曜日: "Weekday",
  "該当日がない月はスキップします。開始日と同じ日付・第n曜日を指定してください。":
    "Months without a matching date are skipped. Use the same date or weekday occurrence as the start date.",
  終了条件: "Ends",
  終了なし: "Never",
  繰り返しの終了日: "Repeat end date",
  回数: "Count",
  繰り返し回数: "Number of occurrences",
  "開催日を確認中…": "Checking dates…",
  "次の開催日（日本時間）": "Upcoming dates (Japan time)",
  設定を適用: "Apply settings",
  その他の色: "Custom color",
  "通知のタイミング (数値)": "Reminder timing (number)",
  "通知のタイミング (単位)": "Reminder timing (unit)",
  この通知を削除: "Delete this reminder",
  通知を追加: "Add reminder",
  通知のメンション先: "Reminder mentions",
  "（ユーザーIDを入力中）": "(entering a user ID)",
  "事前通知・開始時刻の通知で呼びかける相手を、合計10件まで指定できます。 Bot に権限がない場合、通知は届きますが @everyone やメンション不可のロールへの呼びかけは届きません。":
    "Select up to 10 mention targets for advance and start reminders. Without the Bot's permission, the reminder is sent but @everyone and non-mentionable roles will not be notified.",
  "@everyone（全員）": "@everyone (everyone)",
  "全員やメンション不可のロールを指定するには、あなたに「全てのロールにメンション」権限が必要です。":
    "You need “Mention Everyone” permission to select everyone or non-mentionable roles.",
  メンションするロール: "Role to mention",
  "ロールを読み込み中…": "Loading roles…",
  ロールを追加: "Add role",
  "ロールを取得できませんでした。": "Could not load roles.",
  メンションするユーザーID: "User ID to mention",
  "Discord のユーザーID": "Discord user ID",
  正しいユーザーIDを入力してください: "Enter a valid user ID",
  このユーザーはサーバーに参加していません:
    "This user is not a member of this server",
  ユーザーを追加: "Add user",
  "ユーザーIDは Discord の「設定 → 詳細設定 → 開発者モード」を有効にし、相手のメニューから「ユーザーIDをコピー」で取得できます。":
    "Enable Settings → Advanced → Developer Mode in Discord, then use “Copy User ID” in the person's menu.",
  日付を指定して移動: "Jump to date",
  練習用カレンダーで操作を試す: "Try the practice calendar",
  サーバー設定: "Server settings",
  サーバーを切り替え: "Switch server",
  サーバーの凡例: "Server legend",
  このサーバーの予定を隠す: "Hide this server's events",
  このサーバーの予定を表示する: "Show this server's events",
  凡例を折りたたむ: "Collapse legend",
  アカウントメニュー: "Account menu",
  サーバー一覧: "Servers",
  すべての予定: "All events",
  カレンダーの表示設定: "Calendar preferences",
  プッシュ通知: "Push notifications",
  "MCP 接続管理": "MCP connections",
  キーボードショートカット: "Keyboard shortcuts",
  ログアウト: "Sign out",
  メイン: "Main",
  設定: "Settings",
  外部連携: "Integrations",
  サポート: "Support",
  サービス情報: "About",
  管理: "Administration",
  操作を試す: "Try it out",
  ホーム: "Home",
  使い方: "Help",
  サポートサーバー: "Support server",
  更新履歴: "Changelog",
  支援: "Donate",
  利用規約: "Terms of service",
  プライバシーポリシー: "Privacy policy",
  管理コンソール: "Admin console",
  サイト内メニュー: "Site menu",
  メニュー: "Menu",
  サーバー一覧へ: "Go to servers",
  "キーボードショートカット (?)": "Keyboard shortcuts (?)",
  ログイン: "Sign in",
  "DisCalendar ホーム": "DisCalendar home",
  サイト内リンク: "Site links",
  特定商取引法に基づく表記: "Legal disclosure",
  フッタのリンク: "Footer links",
  ライトテーマに切り替え: "Switch to light theme",
  ダークテーマに切り替え: "Switch to dark theme",
  "Discordアカウントでログインして、サーバーのカレンダーを管理できます。":
    "Sign in with Discord to manage your servers' calendars.",
  Discordでログイン: "Sign in with Discord",
  "ログインすると、": "By signing in, you agree to the ",
  と: " and the ",
  "に同意したものとみなします。": ".",
  サーバー選択: "Choose a server",
  カレンダー: "Calendar",
  サーバーを選択: "Choose a server",
  Discordからサーバー一覧を取得できませんでした:
    "Could not load your servers from Discord",
  "再ログインするか、時間をおいて再度お試しください。":
    "Sign in again or try again later.",
  "Bot の参加状況を取得できませんでした。API サーバーが起動しているか確認してください。":
    "Could not check which servers have the Bot. Please try again later.",
  "Bot が参加しているサーバーがありません。":
    "None of your servers have the Bot.",
  "下の一覧から Bot を招待できます。": "Invite the Bot from the list below.",
  "Bot の招待はサーバーの管理者に相談してください。":
    "Ask a server administrator to invite the Bot.",
  "Bot を招待できるサーバー": "Servers you can invite the Bot to",
  "Bot の参加状況を取得できませんでした":
    "Could not check which servers have the Bot",
  "時間をおいて再度お試しください。": "Please try again later.",
  "Bot が参加しているサーバーがありません": "None of your servers have the Bot",
  "サーバー一覧から Bot を招待すると、そのサーバーの予定がここにまとめて表示されます。":
    "Invite the Bot from the servers page to see all their events here.",
  サーバーデータの取得に失敗しました: "Could not load the server",
  "Discord との通信に失敗しました。時間をおいて再度お試しください。":
    "Could not connect to Discord. Please try again later.",
  以下の事項をご確認ください: "Check the following",
  "･ BOTがサーバーに導入されているか": "· The Bot has been added to the server",
  "･ あなた自身がBOTを導入したサーバーに参加しているか":
    "· You are a member of the server with the Bot",
  サーバー選択に戻る: "Back to servers",
  タイトルを入力してください: "Enter a title",
  数値を入力してください: "Enter a number",
  整数で入力してください: "Enter a whole number",
  "色は #RRGGBB 形式で指定してください": "Use the #RRGGBB color format",
  開始日を選択してください: "Select a start date",
  終了日を選択してください: "Select an end date",
  メンション先は10件以内で指定してください: "Select up to 10 mention targets",
  メンション先が重複しています: "Mention targets must be unique",
  メンション先のIDが不正です: "A mention target ID is invalid",
  "Bot がサーバーに参加していません": "The Bot is not a member of this server",
  メンション先のユーザーがサーバーに参加していません:
    "A mentioned user is not a member of this server",
  このサーバーで選択できないロールです:
    "This role cannot be selected in this server",
  "URL は http または https で入力してください": "Use an http or https URL",
  開始時刻を入力してください: "Enter a start time",
  終了時刻を入力してください: "Enter an end time",
  終了日時を開始日時より前にすることはできません:
    "The end cannot be before the start",
  "Discord のイベントにするには終了を開始より後にしてください":
    "A Discord event must end after it starts",
  "1〜100の範囲で入力してください": "Enter a number from 1 to 100",
  通知は10件まで設定できます: "Set up to 10 reminders",
  タイトルは32文字以内で入力してください:
    "Use 32 characters or fewer for the title",
  説明は1000文字以内で入力してください:
    "Use 1,000 characters or fewer for the description",
  "場所 / URL は200文字以内で入力してください":
    "Use 200 characters or fewer for the location / URL",
  分前: { one: "minute before", other: "minutes before" },
  時間前: { one: "hour before", other: "hours before" },
  日前: { one: "day before", other: "days before" },
  週間前: { one: "week before", other: "weeks before" },
  "「{name}」を削除します。この操作は取り消せません。":
    "“{name}” will be deleted. This cannot be undone.",
  "{count} サーバーの予定をまとめて表示": {
    one: "Events from {count} server",
    other: "Events from {count} servers",
  },
  "{count} サーバーの予定をまとめて表示しています。予定の作成・編集は各サーバーのカレンダーで行えます":
    {
      one: "Showing events from {count} server. Create and edit events in each server’s calendar.",
      other:
        "Showing events from {count} servers. Create and edit events in each server’s calendar.",
    },
  "JPEG・PNG・WebP・PDFを選択してください":
    "Select a JPEG, PNG, WebP or PDF file",
  "JPEG・PNG・WebP・PDFを有効なファイル名で指定してください":
    "Choose a JPEG, PNG, WebP or PDF with a valid filename",
  "添付は予定ごとに10件、サーバー全体で1GiBまでです（送信待ちを含む）":
    "Up to 10 attachments per event and 1 GiB per server, including pending uploads",
  "添付の予約期限が切れました。ファイルを選び直してください":
    "The upload reservation has expired. Select the file again",
  "送信されたファイルのサイズ・種類が選択時と異なります":
    "The uploaded file's size or type differs from the selected file",
  "ファイルの内容がJPEG・PNG・WebP・PDFのいずれとも一致しません":
    "The file's contents do not match JPEG, PNG, WebP or PDF",
  添付ファイルの保存先が設定されていません:
    "Attachment storage is not configured",
  "ファイルの保存先に接続できません。時間をおいて再試行してください":
    "Could not connect to attachment storage. Try again later",
  この形式はプレビューできません: "This file type cannot be previewed",
  ファイルは1バイト以上10MiB以下にしてください:
    "Files must be between 1 byte and 10 MiB",
  ファイルを送信できませんでした: "Could not upload the file",
  "1件 {size} バイト（10 MiB）、サーバー全体 1,073,741,824 バイト（1 GiB）まで。空のファイルは添付できません。":
    "Up to {size} bytes (10 MiB) per file and 1,073,741,824 bytes (1 GiB) per server. Empty files cannot be attached.",
  管理権限あり: "Management permissions",
  予定を編集可能: "Can edit events",
  閲覧のみ: "View only",
  今日: "Today",
  前の期間: "Previous period",
  次の期間: "Next period",
  月: "Month",
  週: "Week",
  "4日": "4 days",
  日: "Day",
  リスト: "List",
  "・": ", ",
  通信が中断されました: "The request was cancelled",
  "通信に失敗しました。ネットワークを確認してください":
    "Could not connect. Check your network connection",
  "ログインの有効期限が切れました。再度ログインしてください":
    "Your session has expired. Sign in again",
  この操作を行う権限がありません: "You do not have permission to do this",
  "Bot に「イベントの作成」権限がないため Discord に反映できませんでした。Bot を招待し直すと利用できます (使い方の「サーバーに導入する」)":
    "The Bot needs Discord's “Create Events” permission to sync events. Reinvite it using the server setup guide.",
  "対象が見つかりません (他のユーザーが削除した可能性があります)":
    "Not found. Someone may have deleted it",
  "他の更新と同時に行われたため保存できませんでした。もう一度お試しください":
    "A concurrent update prevented saving. Try again",
  "Discord API の制限中です。しばらく待ってから再度お試しください":
    "Discord is limiting requests. Please try again later",
  "メンバーの確認が多すぎます。1分ほど待ってから再試行してください":
    "Too many member checks. Wait about a minute and try again",
  "Discord との通信に失敗しました。時間をおいて再度お試しください":
    "Could not connect to Discord. Please try again later",
  管理者: "Administrator",
  一般メンバー: "Member",
  "招待 ↗": "Invite ↗",
  今日に戻る: "Go to today",
  "前 / 次の期間へ移動": "Go to the previous / next period",
  "予定を新規作成 (予定を編集できるサーバーのカレンダーのみ)":
    "Create a new event (only in servers where you can edit events)",
  月表示に切り替え: "Switch to month view",
  週表示に切り替え: "Switch to week view",
  "4日表示に切り替え": "Switch to 4-day view",
  日表示に切り替え: "Switch to day view",
  リスト表示に切り替え: "Switch to list view",
  この一覧を表示: "Show this list",
  開いている予定の詳細やダイアログを閉じる: "Close event details or a dialog",
  "カレンダー画面で使えるキー操作です。文字を入力している間やダイアログを開いている間は使えません。":
    "Keyboard shortcuts for the calendar. Shortcuts are disabled while typing or while a dialog is open.",
  元日: "New Year's Day",
  成人の日: "Coming of Age Day",
  建国記念の日: "National Foundation Day",
  天皇誕生日: "Emperor's Birthday",
  春分の日: "Vernal Equinox Day",
  昭和の日: "Showa Day",
  憲法記念日: "Constitution Memorial Day",
  みどりの日: "Greenery Day",
  こどもの日: "Children's Day",
  海の日: "Marine Day",
  山の日: "Mountain Day",
  敬老の日: "Respect for the Aged Day",
  秋分の日: "Autumnal Equinox Day",
  スポーツの日: "Sports Day",
  体育の日: "Health and Sports Day",
  "体育の日（スポーツの日）": "Health and Sports Day (Sports Day)",
  文化の日: "Culture Day",
  勤労感謝の日: "Labor Thanksgiving Day",
  振替休日: "Substitute Holiday",
  休日: "Holiday",
  "休日（祝日扱い）": "Public Holiday",
  即位礼正殿の儀: "Enthronement Ceremony",
  大喪の礼: "State Funeral of the Emperor",
  結婚の儀: "Imperial Wedding Ceremony",
  国民の休日: "Citizens' Holiday",
  オーナー: "Owner",
  サーバー管理可: "Can manage server",
  メンバー: "Member",
  "「{filename}」を削除しますか？": "Delete “{filename}”?",
  "{label}のメンションを削除": "Remove mention for {label}",
  "サーバーを切り替え: {name}": "Switch server: {name}",
  "他 {count} サーバーを表示": {
    one: "Show {count} more server",
    other: "Show {count} more servers",
  },
  "入力内容が正しくありません ({detail})": "Invalid input ({detail})",
  "この機能は現在使えません ({detail})":
    "This feature is currently unavailable ({detail})",
  "サーバーでエラーが発生しました ({status})":
    "A server error occurred ({status})",
  繰り返し予定の開始日時は秒単位で指定してください:
    "Use whole seconds for the repeating event start time",
  繰り返し条件を指定してください: "Choose a repeat schedule",
  開始日の曜日を含む曜日を選択してください:
    "Select weekdays including the start date’s weekday",
  開始日と同じ日付を指定してください:
    "Use the same day of the month as the start date",
  開始日と同じ第n曜日を指定してください:
    "Use the same weekday occurrence as the start date",
  "回数は1〜10000回で指定してください": "Enter 1 to 10,000 occurrences",
  繰り返しの終了日は開始日以降にしてください:
    "The repeat end date must be on or after the start date",
  終了日が不正です: "Invalid end date",
  繰り返し条件を解釈できません: "Could not read the repeat schedule",
  開始範囲が不正です: "Invalid start range",
  終了範囲が不正です: "Invalid end range",
  "繰り返しの計算上限に達しました。期間を短くしてください":
    "Too many occurrences to calculate. Choose a shorter period",
  繰り返しの計算上限に達しました: "Too many occurrences to calculate",
  終了日が範囲外です: "The end date is out of range",
  "終日の繰り返し予定は開始・終了を0時で指定してください":
    "All-day repeating events must start and end at midnight",
  繰り返し予定はDiscordイベントと連携できません:
    "Repeating events cannot be synced with Discord events",
  Discordイベント連携を解除してから繰り返しを設定してください:
    "Disable Discord event sync before adding a repeat schedule",
  "繰り返し条件の変更は「この回以降」を選択してください":
    "Select “This and future occurrences” to change the repeat schedule",
  "過去の開催枠と重なるため、この回以降をこの日へ移動できません。別の開始日か「この回のみ」を選択してください":
    "This move overlaps earlier occurrences. Choose another start date or “This occurrence”",
  開催日時が範囲外です: "The occurrence start date is out of range",
  開催終了日時が範囲外です: "The occurrence end date is out of range",
  "移動先は中止済みの開催日です。別の開始日を指定してください":
    "This date has a cancelled occurrence. Choose another start date",
  前回開いていたビュー: "Last used view",
  日曜日: "Sunday",
  月曜日: "Monday",
  "設定はこのブラウザに記憶されます。週の開始曜日はすぐに反映され、最初に表示するビューは次にカレンダーを開いたときから使われます。":
    "Settings are saved in this browser. The first day of the week changes immediately. The initial view applies the next time you open the calendar.",
  最初に表示するビュー: "Initial view",
  週の開始曜日: "First day of the week",
  "30 分": "30 minutes",
  "1 時間": "1 hour",
  "1 時間 30 分": "1 hour 30 minutes",
  "2 時間": "2 hours",
  "3 時間": "3 hours",
  サーバー設定に従う: "Use server settings",
  自分で指定: "Set my own",
  新規作成の既定値: "Defaults for new events",
  "保存後、新しく予定を作るときに使われます。編集・複製では元の予定の値を使います。":
    "These defaults apply to new events after saving. Editing or duplicating an event uses its original values.",
  既定の色: "Default color",
  クリックで作るときの長さ: "Duration when creating by clicking",
  "時間帯を範囲選択したときは、その範囲を優先します。終日の日付クリックでは、終日を外したときの長さに使います。":
    "Selecting a time range overrides this duration. When clicking an all-day date, this duration applies if you turn off All day.",
  事前通知の既定値: "Default advance reminders",
  既定の事前通知: "Default advance reminders",
  "すべて削除すると事前通知なしになります。サーバーの既定値より優先されます。":
    "Remove all reminders to disable advance reminders. This overrides the server defaults.",
  既定値を保存: "Save defaults",
  "既定値を保存しました。": "Defaults saved.",
  "ロール一覧を再取得し、削除されたロールを外してから保存してください":
    "Reload the roles and remove deleted roles before saving.",
  "予定の編集権限・通知・外部カレンダーへの連携を設定します":
    "Manage event editing permissions, reminders, and external calendar integrations.",
  サーバーの設定の変更には管理権限が必要です:
    "You need management permissions to change server settings.",
  予定の編集を管理権限または指定ロールのあるメンバーに限定する:
    "Only members with management permissions or selected roles can edit events",
  予定の編集権限: "Event editing permissions",
  "予定の追加・編集・削除を、サーバーのオーナーまたは「管理者」「サーバー管理」「ロールの管理」「メッセージの管理」のいずれかの権限、または設定で指定したロールを持つメンバーに限定します。Discord 側で権限を変更したら「再読込」を押してください。変更後の権限とロール一覧を取り直します。":
    "Only the server owner, members with Administrator, Manage Server, Manage Roles, or Manage Messages permissions, or members with selected roles can create, edit, and delete events. After changing permissions in Discord, select Reload to refresh permissions and roles.",
  "編集を許可するロール ({count}/25)": "Roles allowed to edit ({count}/25)",
  "管理権限がなくても、選んだロールのメンバーは予定を追加・編集・削除できます":
    "Members with selected roles can create, edit, and delete events even without management permissions.",
  "ロール一覧を取得できませんでした。「再読込」で再試行してください。変更していないロール設定はそのまま保存されます":
    "Could not load roles. Select Reload to try again. Unchanged role settings will be preserved when saving.",
  選択できるロールはありません: "No roles available.",
  "設定を取り直せませんでした ({error})。古い設定を上書きしないよう、保存はできません。ダイアログを開き直してください":
    "Could not refresh settings ({error}). Saving is disabled to prevent overwriting newer settings. Close and reopen this dialog.",
  再読込: "Reload",
  "Discord への通知": "Discord reminders",
  "Bot が通知先チャンネルに予定の開始時刻と事前通知を投稿します。Discord の /init と同じ設定です。":
    "The bot posts advance reminders and start-time reminders to the selected channel. This is the same setting as /init in Discord.",
  開始時刻に通知する: "Notify at the start time",
  開始時刻の通知: "Start-time reminders",
  "外すと、予定ごとに設定した事前通知だけが届きます。終日予定の 0:00 の通知も届きません。":
    "When unchecked, only each event's advance reminders are sent. Midnight reminders for all-day events are also disabled.",
  "新しい予定に使う初期値です。予定ごとに変更できます":
    "Defaults for new events. You can change them for each event.",
  通知先チャンネル: "Reminder channel",
  "Bot に「チャンネルを見る」「メッセージを送信」「埋め込みリンク」の権限があるテキストチャンネルを選べます。Discord 側でチャンネルや権限を変えた直後は、反映まで 1 分ほどかかることがあります。":
    "Choose a text channel where the bot has View Channel, Send Messages, and Embed Links permissions. Channel or permission changes in Discord may take about a minute to appear.",
  "チャンネルを読み込み中…": "Loading channels…",
  "設定済み (管理権限を持つメンバーだけが確認・変更できます)":
    "Configured (only members with management permissions can view or change it)",
  "未設定 (通知は届きません)": "Not configured (no reminders will be sent)",
  "一覧にないチャンネル (ID: {id})": "Channel not listed (ID: {id})",
  "チャンネルの一覧を取得できませんでした ({error})。通知先以外の設定は保存できます":
    "Could not load channels ({error}). You can still save other settings.",
  "通知先が未設定のため、予定を作っても通知は届きません。チャンネルを選んで保存してください":
    "No reminder channel is configured. Choose a channel and save to receive reminders for your events.",
  "Bot に{permissions}の権限がないため、このチャンネルには通知を投稿できません。チャンネルの権限設定を見直すか、別のチャンネルを選んでください":
    "The bot cannot post reminders here because it lacks {permissions}. Update channel permissions or choose another channel.",
  "{label}の補足": "More about {label}",
  "削除されたロール (ID: {id})": "Deleted role (ID: {id})",
  "Bot に{permissions}の権限がありません":
    "The bot lacks {permissions} permissions.",
  チャンネルを見る: "View Channel",
  メッセージを送信: "Send Messages",
  埋め込みリンク: "Embed Links",
  "コピーできませんでした。URL を選択してコピーしてください":
    "Could not copy the URL. Select it and copy it manually.",
  外部カレンダーで購読する: "Subscribe in an external calendar",
  "URL を知っている人は誰でも予定を読めます。共有する相手にご注意ください":
    "Anyone with this URL can read the events. Share it carefully.",
  "フィード URL": "Feed URL",
  "再発行中…": "Reissuing…",
  再発行: "Reissue",
  "無効化中…": "Disabling…",
  無効化: "Disable",
  "発行中…": "Generating…",
  "フィード URL を発行": "Generate feed URL",
  "まだ発行されていません。管理権限を持つメンバーがこの画面から発行できます":
    "No feed URL has been generated. A member with management permissions can generate one here.",
  "「外部カレンダーで見る」": "View in an external calendar",
  外部カレンダーでの購読: "External calendar subscriptions",
  "Google カレンダーや Apple カレンダーなどに予定を表示できます。反映までの時間は各サービスの更新間隔によります。":
    "Show your events in Google Calendar, Apple Calendar, and other services. Refresh times depend on each service.",
  "フィード URL を無効化しますか?": "Disable the feed URL?",
  "フィード URL を再発行しますか?": "Reissue the feed URL?",
  "今の URL は使えなくなり、購読しているカレンダーには予定が届かなくなります。もう一度使うには改めて発行します":
    "The current URL will stop working, and subscribed calendars will stop receiving events. Generate a new URL to subscribe again.",
  "新しい URL に置き換わり、今の URL は使えなくなります。購読している人には新しい URL を登録し直してもらってください":
    "The current URL will be replaced and stop working. Ask subscribers to register the new URL.",
  月曜: "Monday",
  火曜: "Tuesday",
  水曜: "Wednesday",
  木曜: "Thursday",
  金曜: "Friday",
  土曜: "Saturday",
  日曜: "Sunday",
  "投稿時刻を 00:00〜23:59 で入力してください":
    "Enter a posting time between 00:00 and 23:59.",
  予定のまとめを投稿する: "Post event summaries",
  "通知先チャンネルへ投稿します。時刻は日本時間 (JST) です。この節は「まとめ投稿を保存」で反映します。":
    "Summaries are posted to the reminder channel. Times are in Japan Standard Time (JST). Select Save summaries to apply these settings.",
  "先に通知先を設定してください。上の「通知先チャンネル」を選んで「保存」するか、Discord で /init を実行します。":
    "First configure a reminder channel above and select Save, or run /init in Discord.",
  "まとめ投稿の設定を取得できませんでした。ダイアログを開き直してください。":
    "Could not load summary settings. Close and reopen this dialog.",
  "まとめ投稿の設定を読み込み中…": "Loading summary settings…",
  まとめ投稿の設定: "Summary settings",
  "毎日、今日の予定を投稿する": "Post today's events every day",
  "毎日の投稿時刻 (JST)": "Daily posting time (JST)",
  "毎週、今週の予定を投稿する": "Post this week's events every week",
  毎週の投稿曜日: "Weekly posting day",
  "毎週の投稿時刻 (JST)": "Weekly posting time (JST)",
  "投稿する曜日から7日間の予定をまとめます。":
    "Includes events for seven days starting on the posting day.",
  予定が無い日は投稿しない: "Skip posts when there are no events",
  "毎週の投稿では、7日間に予定が無いときに省略します。外すと「予定はありません」と投稿します。":
    "Weekly summaries are skipped when there are no events in the seven-day period. When unchecked, a message saying there are no events is posted.",
  "まとめ投稿を保存中…": "Saving summaries…",
  まとめ投稿を保存: "Save summaries",
  まとめ投稿の設定を保存しました: "Summary settings saved.",
  この端末: "This device",
  "予定の事前通知と開始時刻の通知を、登録した端末に届けます。通知範囲はすべての端末で共通です。":
    "Receive advance and start-time reminders on registered devices. The event scope applies to all devices.",
  "iPhone / iPad では iOS 16.4 以降でホーム画面に追加してから有効にしてください。":
    "On iPhone or iPad, use iOS 16.4 or later and add this app to your Home Screen before enabling notifications.",
  通知する予定: "Events to notify about",
  参加している全サーバーの予定: "Events from all my servers",
  自分が作った予定だけ: "Only events I created",
  オフ: "Off",
  "作成者が記録されていない古い予定は「自分が作った予定だけ」の対象になりません。":
    "Older events without a recorded creator are excluded from Only events I created.",
  "このブラウザではプッシュ通知に対応していません。":
    "This browser does not support push notifications.",
  "プッシュ通知は現在準備中です。": "Push notifications are not available yet.",
  "通知が拒否されています。ブラウザのサイト設定で通知を許可してください。":
    "Notifications are blocked. Allow them in your browser's site settings.",
  端末名: "Device name",
  この端末で受け取る: "Receive on this device",
  "先に通知する予定を選んでください。":
    "First choose which events to receive notifications for.",
  登録済みの端末: "Registered devices",
  "登録済みの端末 ({count})": "Registered devices ({count})",
  "登録された端末はありません。": "No devices registered.",
  "送信に失敗したため停止中。端末で登録し直してください。":
    "Disabled after delivery failed. Register this device again.",
  "登録日: {date}": "Registered: {date}",
  "{name}の登録を解除": "Unregister {name}",
  解除: "Unregister",
  外部カレンダーを重ねて表示する: "Overlay external calendars",
  "登録した ICS の予定を読み取り専用で表示します。通知や共有には含まれません":
    "Show subscribed ICS events as read-only overlays. They are excluded from reminders and sharing.",
  登録済みの外部カレンダー: "Subscribed external calendars",
  今すぐ取得: "Fetch now",
  "最終取得:": "Last fetched:",
  まだ取得していません: "Not fetched yet",
  "表示中の期間を展開できません:": "Cannot expand the displayed date range:",
  "取得できません: ": "Could not fetch: ",
  外部カレンダーを追加: "Add external calendar",
  外部カレンダーを編集: "Edit external calendar",
  表示名: "Display name",
  表示色: "Display color",
  "外部カレンダーは 5 件まで登録できます":
    "You can subscribe to up to 5 external calendars.",
  "「{name}」の購読を削除しますか？": "Remove the subscription to “{name}”?",
  "予定の作成・変更・削除を外部に通知します。説明を含む予定の内容が登録した URL に送られます。 操作はその場で反映されます。":
    "Notify external services when events are created, updated, or deleted. Event content, including descriptions, is sent to the registered URL. Changes take effect immediately.",
  "署名用シークレット（一度だけ表示）": "Signing secret (shown only once)",
  "安全な場所に保存してください。この画面を閉じると再表示できません。":
    "Save this in a secure place. It cannot be shown again after you close this view.",
  保存したので閉じる: "Saved, close",
  種類: "Type",
  "汎用（JSON）": "Generic (JSON)",
  "Discord Webhook 互換": "Discord Webhook compatible",
  "Webhook を登録（{count}/5）": "Register Webhook ({count}/5)",
  有効: "Enabled",
  無効: "Disabled",
  "連続失敗 {count} 回": {
    one: "{count} consecutive failure",
    other: "{count} consecutive failures",
  },
  無効にする: "Disable",
  有効にする: "Enable",
  "テスト送信を予約しました。結果は配信ログに表示されます。":
    "Test delivery queued. Results will appear in the delivery log.",
  テスト送信: "Send test",
  "古いシークレットは使えなくなります。再生成しますか？":
    "The old secret will stop working. Generate a new one?",
  シークレット再生成: "Regenerate secret",
  "この Webhook と配信ログを削除しますか？":
    "Delete this Webhook and its delivery log?",
  "直近の配信ログ（最大20件）": "Recent deliveries (up to 20)",
  配信履歴はありません: "No deliveries yet.",
  応答なし: "No response",
  "DisCalendar は無料で利用できます。運営を続けるための任意の支援 (ドネーション) を受け付けています。":
    "DisCalendar is free to use. Optional donations help keep the service running.",
  "支援は任意です。支援の有無で利用できる機能は変わりません":
    "Donations are optional and do not affect which features you can use.",
  "決済は Stripe 社の決済ページで行われます。カード情報が DisCalendar に伝わることはありません":
    "Payments are processed by Stripe. DisCalendar does not receive your card details.",
  "支援の性質上、決済完了後の返金はできません":
    "Donations cannot be refunded once payment is completed.",
  寄付金控除など税制上の優遇の対象ではありません:
    "Donations are not eligible for tax benefits.",
  支援のお願い: "Support DisCalendar",
  "DisCalendar は無料で利用でき、今後も無料で提供し続ける予定です。一方で、サーバー代やドメイン代などの運用費は運営者が負担しています。":
    "DisCalendar is free and is intended to remain free. The operator covers hosting, domain, and other operating costs.",
  "DisCalendar を気に入ってくださった方は、任意の支援 (ドネーション) で運営を応援していただけると励みになります。金額は決済ページで選べます。":
    "If you enjoy DisCalendar, an optional donation helps support its operation. Choose an amount on the payment page.",
  "DisCalendar を支援する": "Donate to DisCalendar",
  支援の決済に関する表記は: "For payment details, see",
  をご覧ください: ".",
  "不具合の報告や機能の提案も、大きな支援になります。":
    "Bug reports and feature suggestions also help us. Visit the",
  "へお気軽にお寄せください。いつもご利用ありがとうございます。":
    "to share your feedback. Thank you for using DisCalendar.",
  エラーが発生しました: "Something went wrong",
  トップページへ: "Home",
  "エラー ID: ": "Error ID: ",
  "ページの表示中に問題が発生しました。「再試行」を押しても直らないときは、一度ログアウトしてからもう一度アクセスしてみてください。":
    "There was a problem displaying this page. If Retry does not help, sign out and try opening it again.",
  "MCPは現在停止中です。既存の接続は解除できます。":
    "MCP is currently unavailable. You can still revoke existing connections.",
  "解除後に開始した操作とトークンの更新を拒否します。実行中の操作は取り消せません。Webからのログアウトだけでは接続は解除されません。":
    "Revoking a connection blocks new operations and token refreshes. Operations already in progress cannot be canceled. Signing out of the website does not revoke connections.",
  "接続はありません。AIクライアントから接続すると、ここで許可した内容を確認・解除できます。":
    "No connections yet. Connect from an AI client to review or revoke its permissions here.",
  対象サーバー: "Servers",
  許可した操作: "Allowed operations",
  接続日時: "Connected",
  最終利用: "Last used",
  未使用: "Never used",
  接続を解除: "Revoke connection",
  "サーバー一覧を取得できませんでした。": "Could not load the server list.",
  "MCP 接続を許可": "Authorize MCP connection",
  "接続するクライアント: ": "Connecting client: ",
  "として許可します。選択したサーバー名や予定のタイトル・説明・日時が接続先に公開されます。DiscordのトークンやWebのログイン情報は渡しません。":
    "is authorizing this connection. Selected server names and event titles, descriptions, and dates will be shared with the client. Discord tokens and website login credentials are not shared.",
  "{name} として許可します。選択したサーバー名や予定のタイトル・説明・日時が接続先に公開されます。DiscordのトークンやWebのログイン情報は渡しません。":
    "Authorizing as {name}. Selected server names and event titles, descriptions, and dates will be shared with the client. Discord tokens and website login credentials are not shared.",
  "作成・変更・削除を許可すると、DisCalendarは操作ごとの確認を求めません。接続先の確認設定は変わりません。許可した範囲で予定の閲覧・作成・変更・削除ができます。":
    "If you allow creating, updating, or deleting events, DisCalendar will not ask for confirmation for each operation. The client's confirmation settings remain unchanged. It can read, create, update, and delete events within the permissions you grant.",
  "MCP の情報を読み込めませんでした": "Could not load MCP information",
  "時間をおいて再試行してください。同意画面を開けない場合はクライアントから接続をやり直してください。":
    "Try again later. If the consent screen does not open, restart the connection from your client.",
  "MCP の情報を読み込んでいます…": "Loading MCP information…",
  "ログインを開始できませんでした。接続をやり直してください。":
    "Could not start sign-in. Restart the connection.",
  "MCP 接続のためのログイン": "Sign in to connect with MCP",
  "Discordでログインした後、接続先のクライアントと許可するサーバー・操作を確認します。":
    "After signing in with Discord, review the client, servers, and operations you want to authorize.",
  "ログイン画面へ移動中…": "Opening sign-in…",
  "ログイン画面へ移動しています。": "Opening the sign-in page.",
  ページが見つかりません: "Page not found",
  "お探しのページは存在しないか、移動または削除された可能性があります。URL に間違いがないかお確かめください。":
    "This page does not exist or may have been moved or deleted. Check the URL and try again.",
  "Discord 用予定管理 Bot": "A scheduling bot for Discord",
  "DisCalendarはDiscord用のカレンダーアプリです。予定の作成から通知まで面倒なコマンド操作はほとんど必要ありません。 使い慣れたブラウザから、どこでも予定の追加や編集をすることができます。":
    "DisCalendar is a calendar app for Discord. Create events and set reminders with hardly any commands. Add or edit events from anywhere in your browser.",
  "BOT を導入する": "Add the bot",
  ログインせずに操作を試す: "Try without signing in",
  "OR 既に導入済みの方は": "OR already using it?",
  サポートサーバーへ参加: "Join the support server",
  使い方を見る: "Read the guide",
  "DisCalendar のカレンダー画面。月表示に色分けされた予定が並んでいる":
    "DisCalendar's month view with color-coded events (Japanese interface).",
  "ブラウザから予定を追加・編集": "Create and edit events in your browser",
  "Discord アカウントでログインすると、参加しているサーバーのカレンダーを開けます。日付をクリックして予定を作り、ドラッグで移動。月・週・日の表示を切り替えて、スマホからも操作できます。":
    "Sign in with Discord to open your servers' calendars. Click a date to create an event and drag to reschedule it. Switch between month, week, and day views, including on your phone.",
  "予定の編集ダイアログ。「基本」「繰り返し・通知」「添付・共有」のタブと、タイトル・日時・色・場所・説明の入力欄がある":
    "Event editor with Basic, Repeat & reminders, and Files & sharing tabs, plus title, date, color, location, and description fields (Japanese interface).",
  "編集できる人を制限 (restricted モード)":
    "Control who can edit (restricted mode)",
  "サーバー設定で restricted モードを有効にすると、「管理者」「サーバー管理」「ロールの管理」「メッセージの管理」のいずれかの権限、または編集を許可されたロールを持つメンバーが予定を追加・編集・削除できます。閲覧はメンバー全員ができます。":
    "Enable restricted mode so only members with Administrator, Manage Server, Manage Roles, or Manage Messages permissions, or an approved role, can create, edit, and delete events. All members can view events.",
  "サーバー設定ダイアログ。通知・投稿の言語、編集制限と編集ロール、Discord への通知、今日・今週のまとめ投稿、配信用のフィード URL、重ね表示する外部カレンダーの登録、Webhook の設定がある":
    "Server settings for notification and post language, editing permissions, roles, Discord reminders, daily and weekly summaries, feed URLs, external calendar overlays, and Webhooks (Japanese interface).",
  "Discord へ自動で通知": "Automatic Discord reminders",
  "予定ごとに「30 分前」「1 日前」のような事前通知を最大 10 件まで設定できます。時刻になると Bot がサーバー設定で選んだチャンネルに埋め込みメッセージを投稿するので、リマインドの手間がありません。":
    "Set up to 10 advance reminders per event, such as 30 minutes or 1 day before. The bot posts an embed in your selected channel at the scheduled time.",
  "Discord からも操作できる": "Use it directly from Discord",
  "/create で予定の作成、/list で一覧表示、/init で通知先チャンネルの設定。ブラウザを開かなくても Discord のスラッシュコマンドから同じカレンダーを扱えます。":
    "Use /create to add events, /list to view them, and /init to set the reminder channel. Manage the same calendar through Discord slash commands without opening a browser.",
  できること: "Features",
  "サーバーの予定をひとつのカレンダーにまとめて、通知まで Bot に任せられます。":
    "Keep your server's events in one calendar and let the bot handle reminders.",
  "Bot をサーバーに招待する": "Invite the bot to your server",
  "「BOT を導入する」から Discord の画面でサーバーを選んで追加します。サーバーの管理権限が必要です。":
    "Select Add the bot, then choose a server in Discord. You need permission to manage that server.",
  サーバー設定で通知先を決める: "Choose a reminder channel",
  "Web にログインし、カレンダー右上のサーバー設定で通知先チャンネルを選びます。サーバーのオーナーか「管理者」「サーバー管理」「メッセージの管理」「ロールの管理」のいずれかの権限を持つ人が設定でき、Discord の /init でも変更できます。":
    "Sign in and open Server settings above the calendar to choose a reminder channel. The server owner or members with Administrator, Manage Server, Manage Messages, or Manage Roles permissions can configure it. You can also use /init in Discord.",
  予定を追加する: "Add an event",
  "Discord アカウントでログインし、サーバーを選んでカレンダーを開けば、あとは日付をクリックするだけです。":
    "Sign in with Discord, choose a server, open its calendar, and click a date.",
  はじめかた: "Getting started",
  くわしい手順は: "For detailed instructions, see",
  "を参照してください。": ".",
  "まずは Bot をサーバーに追加してみてください":
    "Start by adding the bot to your server",
  "無料で使えます。困ったときはサポートサーバーで質問してください。":
    "It's free to use. Visit the support server if you need help.",
  "場所：": "Location:",
  サーバーのカレンダーを開く: "Open the server calendar",
  "カレンダーの閲覧にはログインとサーバーへの参加が必要です。":
    "Sign in and join the server to view its calendar.",
  "ログインもBotの導入も不要。練習用カレンダーで予定の作成・編集・通知・削除を体験できます。":
    "No sign-in or bot installation needed. Practice creating, editing, and deleting events and previewing reminders.",
  カレンダーを使ってみよう: "Try the calendar",
  "ログインもBotの導入も不要。まずはここで、予定をひとつ。":
    "No sign-in or bot installation needed. Start with one event.",
  "練習用・Discordには送信されません":
    "Practice only · Nothing is sent to Discord",
  使い方の目次: "Guide contents",
  前後のページ: "Previous and next pages",
  次のページ: "Next page",
  前のページ: "Previous page",
  お困りのときは: "Need help? Contact us on the",
  へご連絡ください: ".",
  追加: "Add",
  コピーしました: "Copied",
  コピー: "Copy",
  "必要な項目がない、または形式が不正":
    "Missing required fields or invalid format",
  "個別変更・除外日を含む繰り返し":
    "Repeating events with modified or excluded occurrences",
  キャンセル済み: "Canceled",
  対応していない繰り返し条件: "Unsupported repeat rule",
  タイトルなし: "Missing title",
  日時が不正: "Invalid date or time",
  タイムゾーンを解決できない: "Unknown time zone",
  毎月: "Monthly",
  "ICS ファイルは 1 MiB 以下にしてください":
    "Choose an ICS file no larger than 1 MiB.",
  ファイルを読み取れませんでした: "Could not read the file.",
  "外部カレンダーの予定を確認し、選んだものをこのサーバーへ追加します。":
    "Review external calendar events and add the selected ones to this server.",
  ICSファイル: "ICS file",
  "1 MiBまで。ファイルは保存されません。":
    "Up to 1 MiB. The file itself is not stored.",
  期間を反映: "Apply date range",
  "{total}件中 {count}件を選択": "{count} of {total} selected",
  "生成見込み {count}件": "Estimated occurrences: {count}",
  取り込み色: "Import color",
  "一度に取り込めるのは200件までです。期間を狭めてください。":
    "You can import up to 200 events at once. Narrow the date range.",
  "繰り返しを含む生成見込みは10,000件までです。選択を減らしてください。":
    "The estimated total, including repeats, cannot exceed 10,000 occurrences. Select fewer events.",
  取り込めない予定: "Events that cannot be imported",
  "{count}件": { one: "{count} item", other: "{count} items" },
  すべて選択: "Select all",
  取り込む予定: "Events to import",
  すでにあります: "Already exists",
  "{field}を切り詰めました": "{field} was shortened",
  "ファイルを確認しています…": "Checking the file…",
  "{count}件の予定を取り込みました。": {
    one: "Imported {count} event.",
    other: "Imported {count} events.",
  },
  "取り込み中…": "Importing…",
  "{count}件を取り込む": {
    one: "Import {count} event",
    other: "Import {count} events",
  },
  許可する操作: "Operations to allow",
  許可するサーバー: "Servers to allow",
  "要求された操作と現在利用できるサーバーをすべて選択しています。不要な項目を外してください。「選択した内容を許可」を押すまで接続は許可されません。":
    "All requested operations and currently available servers are selected. Deselect anything you do not need. The connection is not authorized until you select Allow selection.",
  "{count} / {total} 件選択": "{count} / {total} selected",
  すべて解除: "Deselect all",
  "許可できるサーバーがありません。本人とBotが参加しているサーバーを確認してください。":
    "No servers are available. Check that both you and the bot have joined the server.",
  "許可できる操作が要求されていません。クライアントから接続をやり直してください。":
    "No available operations were requested. Restart the connection from your client.",
  "許可する操作とサーバーをそれぞれ1件以上選んでください。拒否は選択なしでも行えます。":
    "Select at least one operation and one server. You can deny access without selecting anything.",
  "サーバーは100件まで許可できます。不要なサーバーを外してください。":
    "You can allow up to 100 servers. Deselect any you do not need.",
  "後から参加するサーバーは追加されません。接続管理からいつでも解除できます。":
    "Servers you join later are not added automatically. You can revoke access at any time from connection management.",
  選択した内容を許可: "Allow selection",
  拒否: "Deny",
  "処理を完了できませんでした。接続管理で現在の状態を確認してください。同意に失敗した場合はクライアントから接続をやり直してください。":
    "Could not complete the operation. Check its current status in connection management. If authorization failed, restart the connection from your client.",
  "処理中です。このままお待ちください。": "Processing. Please wait.",
  "入力中の変更を破棄しますか？": "Discard unsaved changes?",
  日程調整を編集: "Edit scheduling poll",
  日程調整を作成: "Create scheduling poll",
  "候補を最大{count}件追加できます。日時は日本時間です。":
    "Add up to {count} options. Dates and times are in Japan Standard Time.",
  "日時を変更・削除した候補の回答はリセットされます。":
    "Votes for changed or deleted options will be reset.",
  "タイトルは1〜32文字、説明は1000文字以内で入力してください":
    "Enter a title of 1–32 characters and a description of up to 1,000 characters.",
  締切は未来の日時にしてください: "Choose a deadline in the future.",
  "候補の開始・終了日時と重複を確認してください":
    "Check the options' start and end times and remove duplicates.",
  "締切（任意・日本時間）": "Deadline (optional, Japan time)",
  "説明（任意）": "Description (optional)",
  候補: "Options",
  時刻: "Time",
  "候補 {index}": "Option {index}",
  開始: "Start",
  終了: "End",
  "候補{index}の開始日": "Option {index} start date",
  "候補{index}の終了日": "Option {index} end date",
  "候補{index}の開始時刻": "Option {index} start time",
  "候補{index}の終了時刻": "Option {index} end time",
  "候補{index}を終日にする": "Make option {index} all day",
  "候補{index}を削除": "Delete option {index}",
  候補を追加: "Add option",
  変更を保存: "Save changes",
  進行中: "Open",
  締切済み: "Closed",
  確定済み: "Confirmed",
  "○ 参加できる": "○ Available",
  "△ 未定": "△ Maybe",
  "× 参加できない": "× Unavailable",
  カレンダーに戻る: "Back to calendar",
  "候補日への回答を集め、そのまま予定にできます。最新100件を表示します。":
    "Collect votes on possible dates and turn a chosen option into an event. The latest 100 polls are shown.",
  "締切: ": "Deadline: ",
  "日程調整はありません。": "No scheduling polls.",
  日程調整の一覧に戻る: "Back to scheduling polls",
  日本時間: "Japan time",
  "日程が確定しました。": "The date has been confirmed.",
  カレンダーを見る: "View calendar",
  "投票は締め切られています。集計から予定を確定できます。":
    "Voting has closed. You can confirm an event from the results.",
  候補を編集: "Edit options",
  "投票を締め切りますか？": "Close voting?",
  投票を締め切る: "Close voting",
  "日程調整と回答を削除しますか？ 確定済みの予定は残ります。":
    "Delete this poll and its votes? Confirmed events will remain.",
  日程調整を削除: "Delete scheduling poll",
  "保存先・通知設定を取得できませんでした。":
    "Could not load the destination or reminder settings.",
  再取得: "Reload",
  候補と自分の回答: "Options and your votes",
  "候補ごとに ○ / △ / × を選んでください。何度でも変更できます。":
    "Choose ○, △, or × for each option. You can change your votes at any time.",
  "回答の受付は終了しました。": "Voting has ended.",
  "○が最多": "Most available",
  "{count}人": { one: "{count} person", other: "{count} people" },
  "候補 {index} への回答": "Vote for option {index}",
  この候補で確定: "Confirm this option",
  "選んだ候補で予定を作成します。色・通知・Discordイベント連携を設定できます。":
    "Create an event from the selected option. You can set its color, reminders, and Discord event sync.",
  候補を選択してください: "Choose an option.",
  みんなの回答: "Everyone's votes",
  "{count}人が回答しています。": {
    one: "{count} person has voted.",
    other: "{count} people have voted.",
  },
  "まだ回答はありません。": "No votes yet.",
  候補と参加者の回答表: "Options and participants' votes",
  "○ 参加できる、△ 未定、× 参加できない、— 未回答":
    "○ Available, △ Maybe, × Unavailable, — No vote",
  参加者: "Participant",
  "（自分）": " (you)",
  "Discordの通知チャンネルに案内を投稿しました。":
    "An announcement was posted to the Discord reminder channel.",
  "保存しました。通知チャンネルが未設定のため、Discordへの案内は投稿していません。":
    "Saved. No announcement was posted because no reminder channel is configured.",
  "保存しましたが、Discordへの案内投稿に失敗しました。このページのURLを共有してください。":
    "Saved, but the Discord announcement failed. Share this page's URL instead.",
  "Botの導入や編集権限がなくても、予定の作成から通知まで体験できます。":
    "Try creating events and previewing reminders without installing the bot or needing editing permissions.",
  練習用カレンダーの案内を閉じる: "Dismiss the practice calendar tip",
  まずはカレンダーを見てみよう: "Explore the calendar",
  "ここは練習用のサーバーです。月・週・日の表示を切り替えたり、予定を開いたりしてみましょう。準備ができたら次へ進みます。":
    "This is a practice server. Try switching between month, week, and day views or opening an event. Continue when you're ready.",
  自分の予定を作ってみよう: "Create your own event",
  "「新規作成」からタイトルと日時を入力し、「作成」を押してください。通知を何分前に届けるかも設定できます。":
    "Select New event, enter a title and dates, then select Create. You can also set how far in advance reminders are sent.",
  作った予定を編集してみよう: "Edit your event",
  "予定を開いて「編集」を選び、タイトルや説明、通知のタイミングを変えて「保存」を押してください。":
    "Open the event and select Edit. Change the title, description, or reminder timing, then select Save.",
  Discordに届く通知を見てみよう: "Preview Discord reminders",
  "今の予定で通知の見本を作りました。事前通知と開始時刻の通知を切り替えて確認できます。実際の送信は行いません。":
    "Here is a reminder preview for your event. Switch between advance and start-time reminders. Nothing is actually sent.",
  予定を削除してみよう: "Delete your event",
  "予定を開いて「削除」を選び、確認画面でも「削除」を押してください。実際のサーバーではメンバー全員のカレンダーから消えます。":
    "Open the event, select Delete, and confirm. In a real server, the event disappears from every member's calendar.",
  これで基本の操作は完了です: "You've learned the basics",
  "作成・編集・通知・削除を体験できました。このまま自由に試したり、自分のサーバーで使い始めたりできます。":
    "You've tried creating, editing, previewing reminders, and deleting events. Keep exploring or start using DisCalendar in your own server.",
  "練習用カレンダーを準備しています…": "Preparing the practice calendar…",
  練: "P",
  練習用サーバー: "Practice server",
  予定の日時は日本時間です: "Event dates and times are in Japan Standard Time",
  最初からやり直す: "Start over",
  操作のヒント: "Tips",
  チュートリアル: "Tutorial",
  自由に試せます: "Explore freely",
  "ステップ {step} / {total}": "Step {step} / {total}",
  自分のペースで試してみよう: "Explore at your own pace",
  "予定の作成・編集・削除を自由に試せます。作成・編集した予定の通知もここで確認できます。":
    "Practice creating, editing, and deleting events. You can also preview reminders for events you create or edit.",
  "。この画面を閉じると通知の見本を確認できます。":
    ". Close this dialog to see the reminder preview.",
  詳しい説明を閉じる: "Hide details",
  詳しい説明: "Details",
  "・通知プレビュー": " · Reminder preview",
  "予定を作成すると、通知の見本を確認できます。":
    "Create an event to see its reminder preview.",
  "練習した予定は保存されません。ページを開き直すと最初の状態に戻ります。":
    "Practice events are not saved. Reopening the page resets the calendar.",
  前の説明へ: "Previous step",
  通知を確認した: "I've checked the reminders",
  次へ: "Next",
  自由に試す: "Explore freely",
  案内を終了: "End guide",
  案内を最初から見る: "Restart guide",
  "操作が完了すると、自動で次のステップへ進みます。":
    "The guide advances automatically when you complete the action.",
  通知プレビュー: "Reminder preview",
  "{notification}の通知": "Reminder: {notification}",
  Discord通知の見本: "Discord reminder preview",
  日時: "Date and time",
  "本番ではサーバー設定で通知先チャンネルを選びます。開始時刻の通知もサーバー設定で変更できます。":
    "In your server, choose the reminder channel in Server settings. You can also change start-time reminders there.",
  自分のサーバーで使うには: "Use it in your own server",
  Botを招待する: "Invite the bot",
  サーバー一覧を開く: "Open server list",
  導入手順を見る: "Read setup instructions",
  "通知先は Web のサーバー設定から選べます。Botの招待・通知先の設定や、編集権限の付与ができない場合は、サーバーの管理者に相談してください。":
    "Choose the reminder channel in Server settings on the website. Ask a server administrator if you cannot invite the bot, configure reminders, or grant editing permissions.",
  基本的な使い方: "Getting started",
  "DisCalendar を使い始めるまでの 3 ステップ (Bot の招待、通知先チャンネルの設定、ブラウザからの予定作成)。":
    "Start in three steps: invite the bot, choose a reminder channel, and create events in your browser.",
  "Discord アカウントで DisCalendar にログインする手順と、ログイン時に求められる権限について。":
    "How to sign in with Discord and understand the permissions requested at sign-in.",
  "Bot の招待": "Invite the bot",
  "DisCalendar の Bot を Discord サーバーに追加する手順と、Bot に必要な権限について。":
    "How to add DisCalendar to your Discord server and grant the permissions it needs.",
  初期設定: "Initial setup",
  "Web のサーバー設定または /init コマンドで、予定の通知を受け取るチャンネルを設定する手順。":
    "Choose a reminder channel in Server settings on the website or with the /init command.",
  予定の追加と表示: "Create and view events",
  "ブラウザのカレンダー画面の見方、全サーバーの予定をまとめて見る「すべての予定」、予定ダイアログからの予定の作成 (日時・色・事前通知・説明) について。":
    "Explore the calendar, view events across servers with All events, and create events with dates, colors, reminders, and descriptions.",
  "候補日への回答を集め、選んだ日時で予定を作成する方法。":
    "Collect votes on possible dates and create an event from the chosen option.",
  予定の編集と削除: "Edit and delete events",
  "作成した予定の編集・移動・削除の方法と、サーバー設定で編集できるユーザーを制限する方法。":
    "Edit, move, or delete events and control who can edit them in Server settings.",
  外部カレンダーで見る: "View in external calendars",
  "サーバーの予定を Google カレンダー・Apple カレンダー・Outlook などで購読する方法と、フィード URL の発行・再発行・無効化について。":
    "Subscribe to server events in Google Calendar, Apple Calendar, Outlook, and other apps. Generate, reissue, or disable feed URLs.",
  予定の変更を外部に通知する: "Notify external services",
  "Webhook の登録、Discord への通知、JSON 本文と署名の検証方法。":
    "Register Webhooks, send Discord notifications, and verify JSON payloads and signatures.",
  Codexとの接続: "Connect with Codex",
  "CodexとのMCP接続で予定を確認・作成・編集する方法。権限の選択、接続解除と困ったときの対応。":
    "View, create, and edit events from Codex through MCP. Choose permissions, revoke connections, and troubleshoot problems.",
  利用可能なコマンド: "Available commands",
  "Discord から使えるスラッシュコマンド (/help, /create, /list, /init, /invite) の一覧と使い方。":
    "How to use Discord slash commands: /help, /create, /list, /init, and /invite.",
  サーバー一覧の読み取り: "Read server list",
  予定の読み取り: "Read events",
  予定の作成: "Create events",
  予定の変更: "Update events",
  予定の削除: "Delete events",
  接続を継続して利用: "Maintain access",
  "許可する権限とサーバーを選び直してください。":
    "Select the permissions and servers again.",
  "登録済みの端末 ({count}/10)": "Registered devices ({count}/10)",
  "ページの表示に失敗しました。時間をおいて再度お試しください。":
    "Could not display the page. Try again later.",
  "(日本時間)": "(Japan time)",
  日程調整への回答: "Vote on a scheduling poll",
  "通知が許可されていません。ブラウザのサイト設定を確認してください。":
    "Notifications are not allowed. Check your browser's site settings.",
  "通知の準備中です。ページを再読み込みしてからお試しください。":
    "Notifications are being prepared. Reload the page and try again.",
  "端末の購読情報を取得できませんでした。":
    "Could not retrieve this device's subscription.",
  "以前のアカウントの購読を解除しました。もう一度「この端末で受け取る」を押してください。":
    "The previous account's subscription was removed. Select Receive on this device again.",
  練習用サーバーを選んでください: "Choose the practice server.",
  "予定が見つかりません。別の予定を選んでください":
    "Event not found. Choose another event.",
  "表示名は 1〜32 文字にしてください":
    "Enter a display name of 1–32 characters.",
  "色は #RRGGBB 形式にしてください": "Use the #RRGGBB color format.",
  "この URL は登録済みです": "This URL is already registered.",
  "種類、または Discord Webhook URL が正しくありません":
    "Invalid type or Discord Webhook URL.",
  "URL の確認がタイムアウトしました": "URL verification timed out.",
  "Webhook は1サーバー5件までです":
    "You can register up to 5 Webhooks per server.",
  "有効な Webhook で1分ほど間隔を空けてお試しください":
    "Use an enabled Webhook and wait about a minute before trying again.",
  再取得してください: "Fetch again.",
  取得がタイムアウトしました: "Fetching timed out.",
  接続に失敗しました: "Connection failed.",
  リダイレクトが多すぎます: "Too many redirects.",
  リダイレクト先が不正です: "Invalid redirect destination.",
  取得先がエラーを返しました: "The source returned an error.",
  読み込みに失敗しました: "Failed to read the response.",
  "ICS が 1 MiB を超えています": "The ICS file exceeds 1 MiB.",
  "ICS の文字コードを読み取れません": "Cannot read the ICS character encoding.",
  "ICS を解析できません": "Cannot parse the ICS file.",
  "予定が 200 件を超えています": "The calendar contains more than 200 events.",
  例外付きの繰り返し予定には対応していません:
    "Repeating events with exceptions are not supported.",
  "未対応の予定、または読み取れない予定が含まれています":
    "The calendar contains unsupported or unreadable events.",
  元日時を読み取れません: "Cannot read the original date and time.",
  "表示する予定が 1000 件を超えています。表示期間を短くしてください":
    "More than 1,000 events would be displayed. Shorten the displayed period.",
  繰り返し終了日が不正です: "Invalid repeat end date.",
  繰り返し条件を読み取れません: "Cannot read the repeat rule.",
  "繰り返しの計算上限に達しました。表示期間を短くしてください":
    "The repeat calculation limit was reached. Shorten the displayed period.",
  日時が範囲外です: "The date and time are out of range.",
  夏時間の切り替えで変換できない日時を含んでいます:
    "The calendar contains a date and time that cannot be converted during a daylight-saving transition.",
  "接続または TLS 通信に失敗しました":
    "Connection or TLS communication failed.",
  送信がタイムアウトしました: "Delivery timed out.",
  "送信先が成功以外の HTTP ステータスを返しました":
    "The destination returned an unsuccessful HTTP status.",
  送信が10回連続で失敗しました: "Delivery failed 10 times in a row.",
  "URL が正しくありません": "Invalid URL.",
  "認証情報・フラグメントを含まない HTTP(S) URL を指定してください":
    "Enter an HTTP(S) URL without credentials or a fragment.",
  ホストがありません: "Missing host.",
  ポートがありません: "Missing port.",
  テストクライアントを作成できません: "Could not create the test client.",
  "DNS の解決に失敗しました": "DNS resolution failed.",
  公開インターネットのアドレスだけを指定できます:
    "Only public internet addresses are allowed.",
  "HTTP クライアントを作成できません": "Could not create the HTTP client.",
  "候補は1〜5件にしてください": "Provide 1–5 options.",
  "候補の日時・重複を確認してください。終日は開始・終了日の0時を指定します":
    "Check option dates and duplicates. All-day options must start and end at midnight.",
  締切済みの日程調整は編集できません:
    "Closed scheduling polls cannot be edited.",
  新規候補にIDは指定できません: "New options must not have an ID.",
  別の日程調整の候補は指定できません:
    "You cannot use an option from another poll.",
  "回答はyes / maybe / noで指定してください": "Choose yes, maybe, or no.",
  "確定する日時は選択した候補と揃え、繰り返しなしにしてください":
    "Use the selected option's dates and disable repeating when confirming.",
  予定数が多すぎます: "Too many events.",
  "購読 URL・暗号鍵・端末名を確認してください":
    "Check the subscription URL, encryption keys, and device name.",
  この端末の購読を解除してから登録し直してください:
    "Unregister this device before registering it again.",
  端末は10台まで登録できます: "You can register up to 10 devices.",
  対応していない繰り返し条件です: "This repeat rule is not supported.",
  繰り返し間隔が不正です: "Invalid repeat interval.",
  隔週の週開始曜日は月曜だけに対応しています:
    "Biweekly rules support Monday as the first day of the week only.",
  毎月の日付が不正です: "Invalid day of the month.",
  毎月の第n曜日が不正です: "Invalid monthly weekday occurrence.",
  対応していない毎月の繰り返し条件です:
    "This monthly repeat rule is not supported.",
  "COUNT と UNTIL は同時に指定できません":
    "COUNT and UNTIL cannot be specified together.",
  繰り返し回数が不正です: "Invalid repeat count.",
  繰り返し終了日が開始日時より前です:
    "The repeat end date is before the start.",
  曜日が不正です: "Invalid weekday.",
  期間が不正です: "Invalid date range.",
  "生成される予定は合計 10000 件以下にしてください":
    "The total number of generated events must not exceed 10,000.",
  "ICS ファイルを解析できません: {detail}":
    "Cannot parse the ICS file: {detail}",
  "JSON を読み取れません: {detail}": "Cannot read JSON: {detail}",
};
