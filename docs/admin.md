# 管理コンソール

[README に戻る](../README.md)

パスとコマンドは、特記がなければリポジトリルートを基準に記載する。

運用・障害対応用の画面 (#33)。api の `ADMIN_DISCORD_USER_IDS` (カンマ区切りの Discord ユーザー ID) に含まれるユーザーだけが
web の `/admin` を開け、api の `/admin/*` を呼べる。それ以外は api が 403 を返し、web は 404 を表示する
(判定は api の `AdminUser` extractor に一本化していて、web は `GET /admin/me` の結果で表示を切り替えるだけ)。
管理コンソールからの書き込み操作は `admin_audit_logs` テーブルに記録する (`api/src/models/admin_audit.rs`)。
`/admin/guilds` で全ギルドの一覧・検索、`/admin/guilds/[id]` で (自分が所属していないギルドも含めて) 予定の閲覧・編集・削除と
`restricted` の切替ができる (#35)。カレンダーは `/dashboard/[id]` と同じ部品を admin 用 API (`/admin/guilds/{guild_id}/events`) に向けて使っている。
`/admin/sql` は読み取り専用の SQL コンソールと定型操作 (#36)。SQL は権限を絞った DB ロール `discalendar_sql_console_<DB 名>`
(非 superuser、特権属性・ロールメンバーシップなし、`CONNECTION LIMIT 1`) で**ログインした専用の接続** (api インスタンスを跨いでも 1 本。同時に実行しようとした管理者は空くのを待つ) と `BEGIN READ ONLY` のトランザクションで実行し、10 秒の締切・
先頭 500 行 / 4 MiB (1 セル 4,000 文字) までを返す。SELECT / WITH / VALUES / TABLE / EXPLAIN / SHOW の 1 文だけ受け付ける。
このロールには `public` スキーマのテーブルの SELECT だけを与え、Better Auth の `account` / `session` / `verification` (トークン類) は
権限を外してあるので、`table_to_xml()` のような関数経由でも読めない (api の接続で `SET ROLE` するのではなくこのロール自身で
ログインするので、SQL から `set_config('role', ...)` で api のロールに戻ることもできない)。さらに `EXPLAIN` の実行計画にこれらの表
(とプランナ統計 `pg_statistic` / `pg_stats`。列のサンプル値に実値が入る) が出てくる文は実行前に拒否する (`api/src/models/admin_sql.rs`)。
ロールの作成・パスワード設定 (`BETTER_AUTH_SECRET` から導出するので env は増えない)・権限付与は api の起動時に自動で行う
(compose の Postgres ユーザーは superuser なのでそのまま通る。Better Auth のテーブルを後から作った場合も api の再起動で権限が付く)。
api の接続ユーザーに `CREATEROLE` が無い環境では起動ログに警告が出て `POST /admin/sql` は 503 を返すので、superuser で次を流し、
その接続文字列を api の `SQL_CONSOLE_DATABASE_URL` に設定して再起動する:

```sql
-- ロール名は discalendar_sql_console_<DB 名> (api が自動で作るときと同じ。別のクラスタなら任意の名前でもよい)。
-- 特権属性を持たせず、どのロールのメンバーにもしない (api が実行前に検証し、満たさなければ 503)
CREATE ROLE discalendar_sql_console_discalendar LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS NOINHERIT
  CONNECTION LIMIT 1 PASSWORD '<任意のパスワード>';  -- 接続は常に 1 本
ALTER ROLE discalendar_sql_console_discalendar SET track_activities = off;  -- 他の管理者に実行中の SQL を見せない
GRANT USAGE ON SCHEMA public TO discalendar_sql_console_discalendar;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO discalendar_sql_console_discalendar;
REVOKE ALL ON TABLE account, session, verification FROM discalendar_sql_console_discalendar;
```

書き込みは自由 SQL ではなく定型操作 (指定ギルドの全予定削除、期限切れセッションの削除) として `POST /admin/ops/*` にあり、
SQL の実行 (成功・失敗とも) と定型操作はすべて `admin_audit_logs` に残る (SQL は文字列リテラルと引用識別子を `'…'` / `"…"` に、コメントを除き、キーワードでも既知の
テーブル・列・関数名でもない識別子も `…` にして保存し、貼り付けたトークン等が履歴に残らないようにしている)。

`/admin` のトップは稼働状況の概要 (#37)。件数 (ギルド / 予定 / ユーザー / 今日の通知予定) と、DB の疎通・
`_sqlx_migrations` の適用状況 (未適用・失敗・チェックサム不一致・切り戻しの検出)・api のビルド情報 (コミット SHA / イメージタグ /
起動時刻) を出す。ビルド情報は実行ファイルに焼き込む値なので、`api/Dockerfile` の `ARG GIT_SHA` / `ARG IMAGE_TAG` として
ビルド時に渡す (staging へのデプロイは自動で入れる。`cargo run` や引数なしの `docker compose build` では「不明」と表示される)。
`/admin/guilds` の下部にある「差分を調べる」で Bot の参加ギルド (Discord API の `GET /users/@me/guilds`) と `guilds` テーブルを
突き合わせられる (Bot の停止中に参加・退出があったときのずれの確認用)。
`/admin/users` は Better Auth の `user` / `session` の一覧・検索と強制ログアウト (セッションの全削除。監査ログに残る)。
**セッショントークンや Discord のアクセストークンは API のレスポンスにも画面にも出さない**。
`/admin/audit-logs` で監査ログを操作の種類・実行者で絞り込みながら追える。

ローカルで試すときは `api/.env` に自分の Discord ユーザー ID を入れて api を再起動し、`/dashboard` の左のメニュー (≡) に出る
「管理コンソール」から開く。compose / staging では ルートの `.env` (`ADMIN_DISCORD_USER_IDS`) で渡す。
