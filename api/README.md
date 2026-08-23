# DisCalendar API

DisCalendar の REST API (Rust / actix-web 4 / sqlx 0.9 / PostgreSQL)。
旧実装 (`tmp/DisCalendarV2/api`: actix-web 4 beta + sqlx 0.5 + serenity) を、
web 側の Better Auth 化に合わせて認証方式を変えつつ移行したもの。

## 旧実装からの変更点

| 項目 | 旧 | 新 |
|---|---|---|
| 認証 | Discord のユーザートークンを `access_token` cookie で受け取り、毎回 Discord API で確認 | **Better Auth のセッション cookie を検証**し、共有 DB の `session` / `account` から Discord ユーザー ID を引く。ブラウザに Discord トークンを出さない |
| メンバー確認・権限 | ユーザートークンで `/users/@me/guilds` など 4 回呼び出し | **Bot トークン**で `GET /guilds/{id}` と `GET /guilds/{id}/members/{user}` を呼び、ロールからギルド権限を計算。結果は短時間キャッシュ (moka) |
| restricted モード | クライアント側で表示制御のみ | **サーバー側で強制** (予定の作成・更新・削除を 403) |
| イベント更新/削除 | `event_id` だけで更新 (他ギルドの予定を触れた) | `guild_id` と `event_id` の両方で絞る |
| 予定の取得 | `start_at` + `date_type` で固定幅 | `start` / `end` の範囲指定 (FullCalendar の `fetchInfo` と同じ) で、期間に**重なる**予定を返す |
| 通知設定 | `{"key":0,"num":30,"type":"分前"}` の JSON 文字列配列 | API 上は `{ "num": 30, "unit": "minutes" }`。DB には旧形式のまま保存 (旧 Bot 互換) |
| Snowflake | serenity の `GuildId` (数値) → フロントで json-bigint が必要 | すべて文字列 |
| OpenAPI | なし | utoipa で `/openapi.json`、`/docs/` に Swagger UI |

DB スキーマ (`migrations/`) は旧実装のファイルを**そのまま**引き継いでいる
(`_sqlx_migrations` のチェックサムが一致するよう変更しないこと)。
稼働中の旧 Bot が同じテーブルを読み書きするため、カラムの型変更は Bot 移行後に行う。

## エンドポイント

すべて Better Auth のセッション cookie が必要 (`/`, `/healthz`, `/docs`, `/openapi.json` を除く)。
curl などからは cookie の値をそのまま `Authorization: Bearer <value>` で渡せる。

| メソッド | パス | 内容 |
|---|---|---|
| GET | `/` | バージョン文字列 |
| GET | `/healthz` | DB 疎通込みのヘルスチェック |
| GET | `/guilds/joined?guild_ids=a,b,c` | 指定 ID のうち Bot が参加しているギルド |
| GET | `/guilds/{guild_id}` | ギルド情報 (メンバーのみ) |
| GET | `/guilds/{guild_id}/@me/permissions` | 自分のギルド権限 (`can_manage_server` など) |
| GET | `/guilds/{guild_id}/config` | ギルド設定 (`restricted`) |
| PUT | `/guilds/{guild_id}/config` | ギルド設定の更新 (管理権限が必要) |
| GET | `/events/{guild_id}?start=&end=` | 期間に重なる予定 |
| POST | `/events/{guild_id}` | 予定の作成 (201) |
| PUT | `/events/{guild_id}/{event_id}` | 予定の更新 |
| DELETE | `/events/{guild_id}/{event_id}` | 予定の削除 (204) |
| GET | `/admin/me` | 管理者の確認 (`ADMIN_DISCORD_USER_IDS` 以外は 403)。`/admin/*` は管理コンソール用で、ギルドのメンバーシップを見ない代わりにホワイトリストで制限する (#33) |
| GET | `/admin/guilds?q=&page=` | 全ギルドの一覧・検索 (guild_id 完全一致 / 名前部分一致、50 件ずつ)。Bot 退出後にデータだけ残っているギルドも含む (`registered` = false)。`restricted` / 通知チャンネル / 予定数付き |
| GET | `/admin/guilds/{guild_id}` | ギルド詳細 (+ Bot の参加状況を Discord API で確認) |
| GET | `/admin/guilds/{guild_id}/events?start=&end=` | 任意のギルドの予定 (条件は `/events/{guild_id}` と同じ) |
| POST / PUT / DELETE | `/admin/guilds/{guild_id}/events[/{event_id}]` | 予定の作成・更新・削除。`admin_audit_logs` に変更前後を記録 (変更前は `FOR UPDATE` で読む)。どのテーブルにも無いギルドへの作成は 404 |
| PUT | `/admin/guilds/{guild_id}/config` | `restricted` の切替 (監査ログに記録)。未知のギルドは 404 |
| POST | `/admin/sql` | 読み取り専用 SQL の実行 (`{ "sql": "..." }`)。権限を絞ったロール `discalendar_sql_console` (保護テーブルの権限なし・非 superuser) でログインした専用の接続 + `BEGIN READ ONLY` + 10 秒の締切 + 先頭 500 行 / 4 MiB / 1 セル 4,000 文字 (`truncated` で通知)。SELECT / WITH / VALUES / TABLE / EXPLAIN / SHOW の 1 文のみ。`account` / `session` / `verification` / `pg_statistic` (`pg_stats`) を読む文は `EXPLAIN` の計画を見て実行前に 400。Postgres のエラーも 400 でメッセージをそのまま返す。ロールが使えなければ 503。成功・失敗とも `admin_audit_logs` (`sql.select`) に残す (SQL は文字列リテラルとコメントを伏せて保存) |
| GET | `/admin/sql/history` | SQL コンソールの実行履歴 (全管理者分、新しい順 20 件。監査ログから組み立てる) |
| POST | `/admin/ops/delete-guild-events` | 指定ギルド (`{ "guild_id": "..." }`) の予定をすべて削除。削除した予定は監査ログ (`ops.delete_guild_events`) の `before.events` に先頭 200 件まで残す (`detail.deleted` に件数)。未知のギルドは 404 |
| POST | `/admin/ops/purge-expired-sessions` | Better Auth の期限切れ `session` を削除 (`ops.purge_expired_sessions`) |

エラーは `{ "error": "<kind>", "message": "<説明>" }` (kind: `unauthorized` / `forbidden` / `not_found` /
`bad_request` / `rate_limited` / `unavailable` / `discord_error` / `database_error` / `internal_error`)。

日時は旧実装と同じく**タイムゾーンなしの JST** (`2026-08-22T10:00:00`)。

## 開発

前提: Rust (`rust-toolchain.toml` で 1.98 に固定、rustup が自動で入れる) / ローカルの PostgreSQL /
web 側のセットアップ済み (Better Auth のテーブルが同じ DB にあること)

```sh
cd api
cp .env.example .env   # DATABASE_URL / DISCORD_BOT_TOKEN / BETTER_AUTH_SECRET を設定

cargo run              # 起動時に migrations/ を適用して http://127.0.0.1:8080 で待ち受け
cargo test             # tests/ の DB テストは #[sqlx::test] が一時 DB を作って実行する (Postgres 必須)
cargo clippy --all-targets -- -D warnings
cargo fmt
```

`BETTER_AUTH_SECRET` は `web/.env.local` と同じ値にすること (違うとすべて 401 になる)。
ローカルで Discord 依存のエンドポイント (`/guilds/{id}/...`, `/events/...`) を叩くには
`DISCORD_BOT_TOKEN` に本物の Bot トークンが必要 (未設定だと Discord が 401 を返し、API は 502 `discord_error` になる)。
web にログインした状態なら、ブラウザの cookie `better-auth.session_token` の値をそのまま
`Authorization: Bearer <値>` に付ければ curl / Swagger UI から試せる。

### sqlx のコンパイル時チェック

`query!` / `query_as!` はビルド時に DB でクエリを検証する。

- ローカル: `.env` の `DATABASE_URL` (または環境変数) の DB に接続して検証
- CI / Docker: `SQLX_OFFLINE=true` で `.sqlx/` のキャッシュを使う

クエリを追加・変更したら `cargo sqlx prepare` (要 `cargo install sqlx-cli --no-default-features --features postgres,rustls`)
を実行して `.sqlx/` を更新し、コミットする。

### Docker

```sh
# リポジトリルートで
docker build -f api/Dockerfile -t discalendar-api .
docker run --rm -p 8080:8080 --env-file api/.env discalendar-api
```

## 構成

```
src/
  main.rs           エントリポイント (dotenv, tracing, Config)
  lib.rs            run(): DB 接続・マイグレーション・SQL コンソール用ロールとプールの用意・HttpServer 構築
  config.rs         環境変数
  error.rs          ApiError → JSON エラーレスポンス
  state.rs          AppState (pool / Discord client / auth 設定 / 管理者設定)
  auth.rs           AuthUser extractor (Better Auth の署名付き cookie を検証)
  admin.rs          AdminUser extractor (AuthUser + ADMIN_DISCORD_USER_IDS のホワイトリスト)
  discord/          Bot トークンでの Discord API 呼び出し + 権限計算 + キャッシュ
  models/           sqlx クエリ (events / guilds / guild_config / admin_audit_logs) と通知形式の変換。
                    admin_sql.rs が SQL コンソールの実行 (読み取り専用・タイムアウト・行数上限・保護テーブルの判定)
  routes/           ハンドラ (utoipa の path 定義付き)、GuildMember extractor。
                    admin_guilds.rs / admin_sql.rs / admin_ops.rs が管理コンソールのギルド・予定 / SQL / 定型操作
  openapi.rs        OpenAPI ドキュメント定義
migrations/         旧実装から引き継いだスキーマ (変更禁止、追加は新ファイルで)
```
