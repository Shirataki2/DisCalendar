# DisCalendar Bot

DisCalendar の Discord Bot (Rust / poise 0.6 / serenity 0.12 / sqlx 0.9 / PostgreSQL)。
旧実装 (`tmp/DisCalendarV2/bot`: poise 0.3 + serenity 0.10 + sqlx 0.5 + sentry) をルートの Cargo workspace に移し、
依存を更新しながら段階的に移行している。

## 移行状況

| 機能 | 状態 |
|---|---|
| 起動・DB 接続・ギルド登録イベント (`guilds` テーブルの更新) | 移行済み (#2) |
| スラッシュコマンド (help / create / list / init / invite / register) | 未着手 (#3)。旧版のプレフィックスコマンド `cal ...` を残すかはそこで決める |
| 定期タスク (予定の通知 / presence / 日付アイコン更新) | 未着手 (#4) |
| エラー監視 (Sentry) | #17 で検討 |

## 旧実装からの変更点

| 項目 | 旧 | 新 |
|---|---|---|
| フレームワーク | poise 0.3 / serenity 0.10 (`poise::Event`) | poise 0.6 / serenity 0.12 (`serenity::FullEvent`) |
| ログ | `log` + pretty_env_logger + sentry-log | `tracing` + tracing-subscriber (api と同じ。`RUST_LOG` で制御) |
| 環境変数 | `BOT_TOKEN` / `SENTRY_URL` (必須) | `DISCORD_BOT_TOKEN` (api と同名) / `BOT_LOG_CHANNEL_ID` (任意) |
| 参加時 (`GuildCreate`) | 未登録のときだけ INSERT | 起動時に届く参加済みギルドの分も含めて常に upsert (停止中の名前・アイコン変更を取り戻す) |
| 更新時 (`GuildUpdate`) | キャッシュに旧データがあり、名前かアイコンが変わったときだけ UPDATE | 常に upsert |
| 退出時 (`GuildDelete`) | 常に DELETE | Discord 側の障害 (`unavailable`) のときは消さない |
| `locale` | 常に `ja` で上書き | 新規行だけ既定値 `ja`。既存行の値は上書きしない |
| 参加・退出のログ通知 | コードに埋め込んだチャンネル ID に送信 | `BOT_LOG_CHANNEL_ID` のチャンネルに送信 (未設定なら送らない)。送信失敗は warn ログのみ |
| Gateway インテント | `non_privileged` | 同じ (ギルドイベントには `GUILDS` が必要。コマンド移行時に見直す) |
| 終了処理 | なし | SIGINT / SIGTERM でシャードを閉じてから終了 (docker stop 向け) |

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

動作確認: Bot をテスト用サーバーに招待 (Developer Portal > OAuth2 > URL Generator で `bot` + `applications.commands` スコープ)
すると `guilds` に行が入り、サーバー名・アイコンを変えると更新され、Bot を退出させると消える。
`BOT_LOG_CHANNEL_ID` を設定していれば、参加・退出がそのチャンネルに埋め込みで通知される。

```sh
psql -d discalendar_dev -c "SELECT guild_id, name, avatar_url, locale FROM guilds"
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
  lib.rs            run(): DB 接続・poise Framework 構築・Gateway 接続・シグナル処理
  config.rs         環境変数
  data.rs           Data (pool / ログチャンネル) と Context 型
  error.rs          BotError と poise の on_error
  event.rs          Gateway イベント (Ready / GuildCreate / GuildUpdate / GuildDelete)
  models/           sqlx クエリ (guilds)
tests/              DB テスト (#[sqlx::test])
```
