# DisCalendar サポート用 Bot

DisCalendar のサポートサーバーにだけ入る Discord Bot (Rust / poise 0.6 / serenity 0.12 / sqlx 0.9 / PostgreSQL)。#423 の Phase 1。
サポートサーバーへの問い合わせを記録し、返信漏れをスタッフに知らせる。

## 本体の Bot と分けている理由

本体の Bot (`bot/`) は全サーバーに入るので、メッセージの本文を受け取る MESSAGE_CONTENT 特権インテントを持たせていない
(100 サーバーを超えると Discord の審査が要り、トークンが漏れたときに全サーバーの本文が読めてしまう)。
本文が要る問い合わせ対応は、サポートサーバー 1 か所にしか入らない別の Discord アプリで受ける。1 サーバーだけなら特権インテントを審査なしで有効にできる。

- サポートサーバー (`SUPPORT_GUILD_ID`) 以外のサーバーに入れられたら、すぐに退出する
- 記録するのは許可リスト (`SUPPORT_RECORD_CHANNEL_IDS`) のチャンネルだけ。DM は受け取らない
- DB は本体と別のデータベース (compose では `discalendar_support`)。同じ DB に置くと `_sqlx_migrations` を api と取り合う

## できること

| 機能 | 内容 |
|---|---|
| 記録 | フォーラムの投稿 (1 投稿 = 1 チケット) と、テキストチャンネルの発言を記録する。編集は上書き、削除されたら本文と添付をその場で消す。テキストチャンネル配下のスレッドは記録しない |
| チケットの状態 | `open` (対応待ち) / `waiting_user` (返信待ち) / `resolved` (解決済み)。下の「状態の決まり方」 |
| 未返信の通知 | 利用者の最終発言から 24 時間スタッフが返信していないチケットを、スタッフ用チャンネルに知らせる (10 分ごとに確認。同じ発言では 1 回だけ) |
| `/remind <期間> [メモ]` | 指定した時間 (`30m` / `12h` / `3d` / `1w`、最大 90 日) のあとに、スタッフ用チャンネルでそのチケットを知らせる (設定した人にメンション) |
| `/ticket status <状態>` | チケットの状態を変える |
| 毎朝のまとめ | 毎朝 9:00 (JST) に未解決のチケットの一覧をスタッフ用チャンネルに投稿する (0 件の日は投稿しない) |
| 保存期間 | 本文・添付・投稿のタイトルは、解決から 180 日 (テキストチャンネルは投稿から 180 日) で消す。投稿者 ID・日時・状態は残す。変えるときはプライバシーポリシー (`web/src/content/support/privacy.mdx`) も直す |
| 取りこぼしの補完 | 起動時に、停止中の発言を記録済みの続きから最大 100 件ずつ取り直す (アクティブな投稿と、一度でも記録したテキストチャンネル) |

コマンドはどちらもスタッフ (`SUPPORT_STAFF_ROLE_IDS` のロールを持つ人) だけが使え、チケットのスレッドの中で実行する。
一般の参加者にコマンドを見せたくなければ、サーバー設定 > 連携サービス でコマンドの表示をスタッフのロールに絞る。

### 状態の決まり方

- 利用者が発言すると `open`。解決済みのチケットでも利用者が書けば `open` に戻る (再オープン)
- スタッフが発言すると `waiting_user`。解決済みのチケットにスタッフが書いても解決済みのまま
- フォーラムの「解決済み」タグを付けると `resolved`、外すと最後の発言者に応じて `open` / `waiting_user`
- `/ticket status` で状態を変えると、「解決済み」タグも付け外しする (タグが 5 個付いていて足せないときは DB の状態だけ変える)

タグは名前 (`解決済み`) で引くので、ID の設定は要らない。「不具合 / 質問 / 要望」など他のタグは記録するだけ。

## サポートサーバーと Discord アプリの準備

1. Discord Developer Portal で本体とは別のアプリを作る
   - Bot > **Public Bot をオフ** (アプリの所有者以外が招待できないようにする)
   - Bot > Privileged Gateway Intents > **MESSAGE CONTENT INTENT をオン** (SERVER MEMBERS / PRESENCE は使わない)
   - Bot > Reset Token でトークンを発行し、本番の `.env` の `SUPPORT_BOT_DISCORD_TOKEN` に入れる (リポジトリ・Issue・ログに貼らない)
2. サポートサーバーに招待する。OAuth2 > URL Generator で `bot` + `applications.commands` スコープ、
   権限は チャンネルを見る・メッセージを送信・埋め込みリンク・メッセージ履歴を読む・スレッドの管理 (`permissions=17179954176`)。
   スレッドの管理はフォーラムの投稿の「解決済み」タグを付け外しするのに要る
3. サポートサーバーに用意する
   - フォーラム「質問・不具合報告」と、タグ「不具合 / 質問 / 要望 / 解決済み」
   - スタッフだけが見えるチャンネル (通知の投稿先)。Bot に送信と埋め込みの権限を与える
   - スタッフのロール
4. ID (開発者モードで右クリック > ID をコピー) を環境変数に入れる

## 環境変数

| 環境変数 | 必須 | 内容 |
|---|---|---|
| `SUPPORT_BOT_DISCORD_TOKEN` | ○ | サポート用 Bot のトークン (本体の `DISCORD_BOT_TOKEN` と取り違えないよう別の名前。`DISCORD_BOT_TOKEN` でも読むが、こちらが優先) |
| `DATABASE_URL` | ○ | サポート用のデータベース。無ければ起動時に作る (接続ユーザーに CREATEDB 権限が要る。compose の `discalendar` ユーザーは持っている) |
| `SUPPORT_GUILD_ID` | ○ | サポートサーバーの ID |
| `SUPPORT_RECORD_CHANNEL_IDS` | ○ | 記録するフォーラム・テキストチャンネルの ID (カンマ区切り) |
| `SUPPORT_STAFF_ROLE_IDS` | ○ | スタッフのロールの ID (カンマ区切り) |
| `SUPPORT_STAFF_CHANNEL_ID` | ○ | 通知・リマインド・毎朝のまとめの投稿先 |
| `DATABASE_MAX_CONNECTIONS` | | 既定 5 (compose は 3) |
| `RUST_LOG` / `LOG_FORMAT` | | ログ。`LOG_FORMAT=json` で 1 行 1 JSON (Loki 用) |
| `SUPPORT_BOT_SENTRY_DSN` / `SENTRY_ENVIRONMENT` / `SENTRY_SAMPLE_RATE` | | エラー監視 (`SENTRY_DSN` でも渡せ、そちらが優先)。未設定なら何も送らない |

ギルド ID などはリポジトリに書かない (公開リポジトリのため)。

## 開発

前提: Rust / ローカルの PostgreSQL。

```sh
cd support-bot
cp .env.example .env   # DATABASE_URL は本体とは別の discalendar_support_dev を指す
cargo run              # DB の作成とマイグレーションの適用をしてから Discord に接続する
cargo test             # tests/ は #[sqlx::test] が一時 DB を作って support-bot/migrations を適用する
cargo clippy --all-targets -- -D warnings
```

クエリを追加・変更したら `support-bot/` で `cargo sqlx prepare -- --all-targets` を実行して `support-bot/.sqlx/` をコミットする
(CI / Docker は `SQLX_OFFLINE=true`)。マクロの接続先は `support-bot/.env` の `DATABASE_URL`。
本体の DB を指す `DATABASE_URL` を環境変数で export したまま `cargo build --workspace` すると、support-bot のクエリまでその DB で検査されて
コンパイルに失敗するので、ワークスペース全体を回すときは `SQLX_OFFLINE=true` を付ける (`.agents/scripts/pre-pr-check.sh` はそうしている)。

### 動作確認

テスト用の Discord サーバーと、テスト用のアプリ (本番と別のトークン) で試す。同じトークンの Bot が他で動いていると二重に記録・通知する。

1. 起動すると `discalendar_support_dev` が無ければ作られ、マイグレーションが当たる
2. 別のサーバーに招待すると、すぐに退出する (ログに `joined an unexpected guild, leaving`)
3. フォーラムに投稿すると `support_threads` と `support_messages` に行が入る。スタッフのロールで返信すると `waiting_user`、
   「解決済み」タグを付けると `resolved`、利用者がもう一度書くと `open` に戻りタグが外れる
4. 発言を編集・削除すると、本文が上書き・消去される
5. `/remind 1m` で 1 分後にスタッフ用チャンネルに通知が届く。`/ticket status` で状態とタグが変わる。スタッフ以外は拒否される
6. 未返信の通知・毎朝のまとめは、DB の時刻をずらして確かめる

```sh
psql -d discalendar_support_dev -c "SELECT thread_id, status, tags, last_user_msg_at, last_staff_msg_at FROM support_threads"
psql -d discalendar_support_dev -c "UPDATE support_threads SET last_user_msg_at = now() - interval '25 hours', reminded_at = NULL WHERE status = 'open'"
psql -d discalendar_support_dev -c "DELETE FROM support_task_runs"   # 9:00 以降なら次の tick で毎朝のまとめを投稿する
```

### Docker

```sh
# リポジトリルートで
docker build -f support-bot/Dockerfile -t discalendar-support-bot .
docker compose --profile support-bot up -d support-bot
```

## 戻し方

本体の DB には触れないので、`api/rollback/` は要らない。止めるときは本番の `COMPOSE_PROFILES` から `support-bot` を外して
`docker compose up -d --remove-orphans`。記録ごと消すなら、止めてから `DROP DATABASE discalendar_support` (R2 のバックアップは残る)。

## 構成

```
src/
  main.rs           Sentry / tracing の初期化 (bot と同じ形)
  lib.rs            DB の作成・マイグレーション・poise の組み立て・Gateway 接続
  config.rs         環境変数
  event.rs          Gateway イベント (ギルドの退出、メッセージ・スレッドの記録、タグからの状態反映)
  record.rs         記録対象の判定と、Discord の型から記録の形への変換
  ticket.rs         チケットの状態と「解決済み」タグの対応
  db.rs             DB の読み書き (Discord へは送らない)
  commands.rs       /ticket status, /remind
  backfill.rs       起動時の取りこぼし補完
  tasks/            未返信の通知・リマインド・毎朝のまとめ・保存期間の削除
migrations/         サポート用データベースのスキーマ (api/migrations とは別系統)
```

## 今後 (別 Issue)

- Phase 2: Claude から読む読み取り専用の MCP (または Markdown への書き出し)。本体 DB の読み取り専用ロールもここで用意する
- Phase 3: 「Issue 化」ボタン (下書き → スタッフ確認 → 起票) と、Claude API による要約・分類・返信案 (スタッフにだけ表示)
