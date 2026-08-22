# DisCalendar Bot

DisCalendar の Discord Bot (Rust / poise 0.6 / serenity 0.12 / sqlx 0.9 / PostgreSQL)。
旧実装 (`tmp/DisCalendarV2/bot`: poise 0.3 + serenity 0.10 + sqlx 0.5 + sentry) をルートの Cargo workspace に移し、
依存を更新しながら段階的に移行している。

## 移行状況

| 機能 | 状態 |
|---|---|
| 起動・DB 接続・ギルド登録イベント (`guilds` テーブルの更新) | 移行済み (#2) |
| スラッシュコマンド (help / create / list / init / invite / register) | 移行済み (#3)。旧版のプレフィックスコマンド `cal ...` は廃止 (下記) |
| 定期タスク (予定の通知 / presence / 日付アイコン更新) | 移行済み (#4) |
| エラー監視 (Sentry) | #17 で検討 |

## コマンド

| コマンド | 内容 | 権限 |
|---|---|---|
| `/help` | 使い方と招待 URL を埋め込みで表示 (本人にだけ見える) | – |
| `/create` | 予定の作成。名称 / 開始・終了の年月日時分 (必須)、説明 / 終日 / 色 / 事前通知 4 つまで (任意)。`events` に api と同じ形式で保存するので web のカレンダーにそのまま出る | restricted モードのサーバーでは管理権限 (下記) が必要 |
| `/list [範囲]` | 予定の一覧 (`過去` / `未来` (既定) / `全て`)。4 件ずつ「前へ」「次へ」でページ送り、「完了」で消える。web で作った予定も出る | – |
| `/init [チャンネル]` | 予定の通知先チャンネルを設定 (省略時は実行したチャンネル)。Bot にそのチャンネルでの「チャンネルを見る」「メッセージを送信」「埋め込みリンク」の権限がなければ保存せず理由を返す。`event_settings` に保存し、通知タスク (`tasks/notify.rs`) が読む | 管理権限が必要 |
| `/invite` | Bot の招待 URL を表示 | – |
| `@DisCalendar register` | スラッシュコマンドを Discord に登録・削除するボタンを出す (このサーバーだけ / グローバル)。help には出ない | Bot のオーナー |

「管理権限」は api / web と同じく「管理者」「サーバー管理」「メッセージの管理」「ロールの管理」のいずれか
(ギルドレベルの基本パーミッション。オーナーは常に可)。

予定の保存形式 (タイムゾーンなしの JST、通知は `{"key":0,"num":30,"type":"分前"}` の JSON 文字列配列、
終日予定は開始日 0:00 〜 終了日 0:00) は api (`api/README.md`) と同じで、`models/notifications.rs` が api 側と同じ変換を持つ。

## 旧実装からの変更点

| 項目 | 旧 | 新 |
|---|---|---|
| フレームワーク | poise 0.3 / serenity 0.10 (`poise::Event`) | poise 0.6 / serenity 0.12 (`serenity::FullEvent`) |
| ログ | `log` + pretty_env_logger + sentry-log | `tracing` + tracing-subscriber (api と同じ。`RUST_LOG` で制御) |
| 環境変数 | `BOT_TOKEN` / `SENTRY_URL` (必須) / `INVITATION_URL` | `DISCORD_BOT_TOKEN` (api と同名) / `BOT_LOG_CHANNEL_ID` (任意) / `DISCORD_BOT_INVITE_URL` (任意、web と同名。未設定ならアプリケーション ID から組み立てる) |
| プレフィックスコマンド `cal help` など | あり (`prefix: "cal"`) | 廃止。メッセージ本文の取得には MESSAGE_CONTENT 特権インテントが必要で、旧版も実際には届いていなかった。`register` だけは Bot へのメンション (本文が届く) で呼ぶ |
| コマンドの登録 | `cal register [global]` | `@DisCalendar register` → poise のボタン UI (サーバー登録 / サーバー削除 / グローバル登録 / グローバル削除) |
| `/create` の引数の並び | 説明 (任意) が必須の引数より前 | Discord の制約どおり必須 (名称・開始・終了) を先に、任意 (説明・終日・色・通知) を後ろに |
| `/create` の色 | 旧 Bot 独自の 10 色 (白・茶など) | web のカラーピッカーの swatch から 10 色 (赤 / 青 (既定) / 緑 / 黄 / 紫 / 水色 / 橙 / ピンク / 灰 / 黒)。web 側でも同じ色として選べる |
| `/create` の終日予定 | 入力した時刻をそのまま保存 | 時・分を無視して開始日 0:00 〜 終了日 0:00 に正規化 (web と同じ表現) |
| `/create` の検証 | 年月日の範囲チェックのみ | 名称 32 文字 / 説明 1000 文字 (api と同じ上限)、存在しない日付、終了 < 開始、同じ通知の重複を弾く。restricted 判定は api と同じ 4 権限 |
| `/list` の「現在」 | UTC の naive 時刻 (JST の予定と 9 時間ずれていた) | JST (api の `now_jst` と同じ) |
| `/list` のページ送り | ボタンは操作がなくても残る | 10 分操作がなければボタンを外す。端のページでは「前へ」「次へ」を無効化 |
| エラー表示 | `Debug` 出力をログに出すだけ (ユーザーには無反応) | 入力・権限の問題は本人にだけ日本語で返す (`BotError::User`)。それ以外はログに出して「予期せぬエラー」とだけ返す |
| `/init` の保存前チェック | なし (Bot が投稿できないチャンネルも保存できた) | Bot 自身のチャンネル内権限 (ロール + 権限上書き、スレッドは親チャンネル) を確認し、足りなければ保存せず足りない権限を返す |
| 初回応答 | 処理が終わってから返信 (3 秒を超えると interaction が失敗扱いになり、`/create` は保存だけ残る) | `/create` (入力検証の後) / `/list` / `/init` は DB や Discord API を呼ぶ前に `defer` で「処理中」を返す。defer 後の返信は公開メッセージになるので、本人にだけ見せたい権限エラーは defer 前 (check 関数・入力検証) で返す |
| 参加時 (`GuildCreate`) | 未登録のときだけ INSERT | 起動時に届く参加済みギルドの分も含めて常に upsert (停止中の名前・アイコン変更を取り戻す) |
| 更新時 (`GuildUpdate`) | キャッシュに旧データがあり、名前かアイコンが変わったときだけ UPDATE | 常に upsert |
| 退出時 (`GuildDelete`) | 常に DELETE | Discord 側の障害 (`unavailable`) のときは消さない |
| 停止中の退出 | 行が残ったまま | `Ready` のたびに `GET /users/@me/guilds` で参加一覧を取り直し、DB にだけ残っている行を消す |
| `locale` | 常に `ja` で上書き | 新規行だけ既定値 `ja`。既存行の値は上書きしない |
| 参加・退出のログ通知 | コードに埋め込んだチャンネル ID に送信 | `BOT_LOG_CHANNEL_ID` のチャンネルに送信 (未設定なら送らない)。送信失敗は warn ログのみ |
| Gateway インテント | `non_privileged` | 同じ (ギルドイベントには `GUILDS`、メンションでの `register` には `GUILD_MESSAGES` が必要で、どちらも含まれる) |
| 終了処理 | なし | SIGINT / SIGTERM でシャードを閉じてから終了 (docker stop 向け) |
| 定期タスクの起動 | `CacheReady` (+ `AtomicBool` で二重起動を防止) | `ShardsReady` (全シャードが `Ready` を受け取った後に一度だけ発火) + `Data::mark_tasks_started` で二重起動を防止 |
| presence の案内文言 | `cal help` / `/help` / サーバー数 / URL を順送り | 廃止済みの `cal help` は出さず、`/help` / サーバー数 / URL の3つを順送り |
| presence の切り替え API | `ctx.set_presence(...).await` (非同期) | serenity 0.12 で同期 API に変更 (`ctx.set_presence(...)`) |
| presence の起動対象 | 単一の Gateway 接続前提 | `Context::set_presence` はそのシャードの接続にしか反映されないため、シャードごとの `Ready` から起動。re-identify を伴う再接続で同じシャードに `Ready` が再送されたときは古いループ (無効な接続を握ったまま) を中断して新しい `Context` のものに置き換える (`Data::replace_presence_task`) |
| アイコン更新の対象サーバー | サポートサーバー ID をコードにハードコード | `BOT_SUPPORT_GUILD_ID` (任意。未設定ならサーバーアイコンの更新をスキップ) |
| アイコン画像の読み込み | 手動で base64 エンコード (`base64` crate) | `serenity::CreateAttachment::path` (base64 化は `EditProfile` / `EditGuild` が内部で行う) |
| アイコン更新のタイミング | tick が JST 0:00 台に来たときだけ更新 (0:01 以降の起動や日付を跨ぐ停止では翌日まで古いまま) | 「最後に更新した日付」を保持し、起動直後や日付が変わった最初の tick で当日分が未反映なら即座に更新 |
| 通知タスクの判定窓 | 固定1分窓 (`[now - 1分, now)`)。1回の実行が60秒を超えると未判定区間が生じ得る | 前回チェック時刻を引き継ぐ可変長の窓 (`[last_checked, now)`) で、実行が遅延しても取りこぼさない |
| 通知の重複排除 | なし | 同じ「num unit 前」が複数保存されていても送信前に一本化 |
| 通知失敗時の安全網 | なし | チャンネル削除・権限剥奪など既知の恒久エラーは即座に処理済み扱い。未知の恒久エラーで送信が失敗し続けても `MAX_SEND_ATTEMPTS` 回で諦め、`last_checked` の凍結による全ギルドの長期停滞を防ぐ |
| DB 障害復旧時の一括送信対策 | なし | DB 障害が数時間続いた後の復旧直後は `[last_checked, now)` が数時間幅のまま一括評価され得るが、発火時刻から `MAX_NOTIFICATION_STALENESS` (15分) 以上経過した通知は陳腐化したとみなして送信せず処理済み扱いにする |
| 祝日判定 | `jpholiday = "0.1"` | `jpholiday = "0.2"` (API 変更: `Date::new(year, month, day)` を使うフリー関数に) |

DB スキーマは api (`api/migrations/`) が正で、Bot はマイグレーションを実行しない。
`guilds` テーブルは web のサーバー選択 (`GET /guilds/joined`) が「Bot 参加済み」の判定に使う。

## 開発

前提: Rust (`rust-toolchain.toml` で固定、rustup が自動で入れる) / ローカルの PostgreSQL /
api を一度起動してマイグレーションが適用済みであること

```sh
cd bot
cp .env.example .env   # DATABASE_URL / DISCORD_BOT_TOKEN は api と同じ値

cargo run              # Discord に接続し、参加中のサーバーを guilds テーブルに反映する
cargo test             # tests/ は #[sqlx::test] が一時 DB を作って api/migrations を適用する (Postgres 必須)
cargo clippy --all-targets -- -D warnings
cargo fmt
```

### 動作確認

1. Bot をテスト用サーバーに招待する (Developer Portal > OAuth2 > URL Generator で `bot` + `applications.commands` スコープ。
   `/invite` が出す URL と同じ)。`guilds` に行が入り、サーバー名・アイコンを変えると更新され、Bot を退出させると消える。
   `BOT_LOG_CHANNEL_ID` を設定していれば、参加・退出がそのチャンネルに埋め込みで通知される
2. スラッシュコマンドを登録する: テスト用サーバーで Bot をメンションして `@DisCalendar register` と送り、
   「Register in guild」を押す (Bot のオーナー = Developer Portal のアプリ所有者のみ)。
   サーバー登録は即時に反映される。本番はグローバル登録を使う (反映に最大 1 時間)。
   両方に登録すると同じコマンドが 2 つ並ぶので、確認後は「Delete in guild」でサーバー側を消す
3. `/init` → `/create` → `/list` の順に試す。`/create` で作った予定は web のカレンダーに表示され、
   web で作った予定は `/list` に出る。web のサーバー設定で restricted を ON にすると、
   管理権限のないユーザーの `/create` は拒否される
4. 定期タスク: `/init` の後、数分後に開始する予定を分前通知付きで `/create` すると、通知タイミングで
   `/init` したチャンネルに embed が届く (終日予定・複数通知・前日通知でも試す)。プレゼンスは起動から
   数十秒で「/help」→「N servers」→「discalendar.app」の順に切り替わる。日付入りアイコンは
   Bot 起動直後の最初の tick (当日分が未反映なら) と、JST の日付が変わった直後の tick で更新される。
   `BOT_SUPPORT_GUILD_ID` を設定して Bot を再起動すればすぐに確認できる

```sh
psql -d discalendar_dev -c "SELECT guild_id, name, avatar_url, locale FROM guilds"
psql -d discalendar_dev -c "SELECT guild_id, channel_id FROM event_settings"
psql -d discalendar_dev -c "SELECT id, name, notifications, is_all_day, start_at, end_at FROM events ORDER BY id DESC LIMIT 5"
```

### sqlx のコンパイル時チェック

api と同じ仕組み (`api/README.md`)。クエリを追加・変更したら `bot/` で

```sh
cargo sqlx prepare -- --all-targets   # tests/ のクエリも含めて bot/.sqlx/ を更新する
```

を実行してコミットする (CI / Docker は `SQLX_OFFLINE=true` で `bot/.sqlx/` を参照する)。

### Docker

```sh
# リポジトリルートで
docker build -f bot/Dockerfile -t discalendar-bot .
docker run --rm --env-file bot/.env discalendar-bot
```

## 構成

```
src/
  main.rs           エントリポイント (dotenv, tracing, Config)
  lib.rs            run(): DB 接続・poise Framework 構築 (コマンド登録・pre_command)・Gateway 接続・シグナル処理
  config.rs         環境変数
  data.rs           Data (pool / ログチャンネル / 招待 URL / サポートサーバー ID) と Context 型
  error.rs          BotError (User = 本人に返す入力・権限エラー) と poise の on_error (日本語の返信)
  event.rs          Gateway イベント (Ready / ShardsReady / GuildCreate / GuildUpdate / GuildDelete)
  checks.rs         管理権限の判定 (api の can_manage_server と同じ)
  paginator.rs      /list のページ送り (ボタン付き埋め込み)
  commands/         スラッシュコマンド (help.txt は /help の本文)
  models/           sqlx クエリ (guilds / events / event_settings / guild_config) と通知形式の変換 (notifications)
  tasks/            定期タスク (notify = 予定の通知 / presence = ステータス表示 / icon_updater = 日付アイコン)
tests/              DB テスト (#[sqlx::test])
assets/             icon_updater が使う日付入りアイコン画像 (01.png 〜 31.png、土曜は _b、日曜・祝日は _r)
```
