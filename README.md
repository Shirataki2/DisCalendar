# DisCalendar

> <https://discalendar.app>

Discord 用のカレンダーアプリ。予定の作成から通知まで、ブラウザから面倒なコマンド操作なしで扱える。

このリポジトリは稼働中の DisCalendar（旧コード: `tmp/DisCalendarV2`、git 管理外）をモダナイズするプロジェクト。最終的に front / api / bot をこのリポジトリ配下に集約する。

## 構成

| ディレクトリ | 内容 | 状態 |
|---|---|---|
| `web/` | Next.js 16 (App Router) + React 19 + FullCalendar v7 + Better Auth + TanStack Query + shadcn/ui + React Hook Form / Zod | LP (静的生成、OGP / favicon)、PWA (manifest / Service Worker)、使い方ページ・規約ページ (MDX、静的生成)、Discord ログイン、サーバー選択、カレンダー (予定の取得 / 作成・編集ダイアログ / 移動 / 削除)、サーバー設定ダイアログ (restricted モード) |
| `api/` | Rust API（actix-web 4 + sqlx 0.9、旧版から移行） | 移行済み（[README](api/README.md)） |
| `bot/` | Discord Bot（poise 0.6 + serenity 0.12 + sqlx 0.9、旧版から移行中） | 基盤 (起動 / DB 接続 / ギルドの参加・退出・更新を `guilds` に反映) とスラッシュコマンド (help / create / list / init / invite / register) を移行済み（[README](bot/README.md)）。定期タスク (#4) は未着手 |
| `docs/` | 技術選定・設計ドキュメント | [技術選定](docs/tech-stack-selection.md) |

Rust 側（api / bot）はルートの `Cargo.toml` を workspace とし、`rust-toolchain.toml` で toolchain を固定している。

### web と api のつなぎ方

- ブラウザからの API 呼び出しは同一オリジンの `/local/api/*` に投げ、`web/next.config.ts` の rewrites が
  `API_URL`（既定 `http://127.0.0.1:8080`）へプロキシする。Better Auth のセッション cookie がそのまま転送され、
  API 側がそれを検証する（旧実装の `@nuxtjs/proxy` と同じ構成）。
- Server Component からは `web/src/lib/api/server.ts` が cookie を付けて API を直接呼ぶ。
- クライアント側のデータ取得・更新は TanStack Query（`web/src/lib/query/`）。ドラッグ移動などは楽観的更新し、失敗時は元に戻す。
- API のエンドポイント定義は `web/src/lib/api/endpoints.ts`（Rust 側の `api/src/routes` と対応）。

### web の UI 部品

- UI 部品は shadcn/ui（`web/components.json`、現行の既定どおり Base UI ベース）。生成物は `web/src/components/ui/` に置き、
  `pnpm dlx shadcn@latest add <name>` で追加する。配色はダーク固定（`globals.css` の `.dark` を `<html class="dark">` で常時適用）。
- 予定の作成・編集ダイアログは `web/src/components/event-form-dialog.tsx`（React Hook Form + Zod）。
  スキーマと API との変換は `web/src/lib/event-form.ts` にあり、上限値（タイトル 32 文字、説明 1000 文字、通知 10 件）は
  `api/src/models/events.rs` と揃える。
- 予定をクリックすると概要ポップオーバー（`event-popover.tsx`、旧 SimpleEdit.vue 相当）から編集・削除できる。
- サーバー設定ダイアログは `web/src/components/guild-settings-dialog.tsx`（旧 ServerSetting.vue 相当）。restricted
  （予定の編集を管理権限を持つユーザーに限定）の切り替えと、Discord 側で権限を変えた後の「再読込」ができる。
  ギルド設定と自分の権限は `dashboard/[id]/page.tsx`（RSC）が取得して TanStack Query に hydrate し、
  `guild-dashboard.tsx` がそこから編集可否を求めるので、保存するとカレンダーの編集可否がその場で切り替わる。
- ダッシュボード（`/dashboard`、`/dashboard/[id]`）の枠は `app/dashboard/layout.tsx` → `dashboard-shell.tsx`
  （アプリバー + ナビゲーションドロワー。PC では常設サイドバーで開閉を cookie `dashboard_sidebar` に覚え、スマホでは Sheet）、
  `dashboard-nav.tsx`（リンク一覧。旧 NavDrawer.vue と同じ並び）、`dashboard-footer.tsx`（固定フッタ。バージョンは
  `web/package.json` の `version` で api / bot の Cargo.toml と揃える）、`user-menu.tsx`（アバターのドロップダウン）。

### web の使い方ページ (docs)

- `/docs/<slug>` は `@next/mdx` で静的生成する（旧 `@nuxt/content` の 7 ページと同じ URL）。本文は `web/src/content/docs/<slug>.mdx`、
  ページの一覧と並び順（サイドナビ・前後リンク・`generateStaticParams`）は `web/src/lib/docs.ts`、
  描画は `web/src/app/docs/[slug]/page.tsx`（`dynamicParams = false`）と `web/src/app/docs/layout.tsx`。
- MDX の見出し・リンクなどの見た目は `web/src/mdx-components.tsx`、本文用の部品（スクリーンショット、ボタン、注意書き、手順、コマンドカード）は
  `web/src/components/docs/`。表を使うため `remark-gfm` を入れている（Turbopack にはパッケージ名の文字列で渡す）。
- スクリーンショットは `web/src/assets/docs/`（`next/image` の静的 import）。LP と共用のものは `web/src/assets/lp/`。
  ページを増やすときは `.mdx` を足して `DOC_PAGES` に 1 行追加する。

### web の規約ページ (利用規約 / プライバシーポリシー)

- `/support/tos` と `/support/privacy` は docs と同じ `@next/mdx` で静的生成する（旧実装と同じ URL）。本文は
  `web/src/content/support/<slug>.mdx`、タイトル・要約・最終更新日は `web/src/lib/support.ts` の `SUPPORT_PAGES`、
  描画は `web/src/app/support/[slug]/page.tsx`（`dynamicParams = false`）と `web/src/app/support/layout.tsx`。
- 導線はヘッダ・フッタ（`site-header.tsx` / `site-footer.tsx`）とログイン画面（`app/login/page.tsx`）から。
- プライバシーポリシーは実装に合わせて書いてあるので、取得する情報（Better Auth のスコープ、保存するカラム）や
  Cookie の使い方、アクセス解析の導入を変えたときは本文と `updatedAt` を更新する。

### web の PWA (manifest / Service Worker)

- マニフェストは `web/src/app/manifest.ts`（`/manifest.webmanifest`。名前・説明・`theme_color` は `web/src/lib/site.ts`）。
  アイコンは `web/public/icons/`（通常の 192 / 512 と、Android の adaptive icon 用に余白を付けた `icon-maskable-*`）。
  iOS のホーム画面用アイコンは `web/src/app/apple-icon.png`。
- Service Worker は [Serwist](https://serwist.pages.dev/) の `@serwist/turbopack`（`@serwist/next` は webpack プラグインなので
  Turbopack の Next 16 では使えない）。本体は `web/src/app/sw.ts`、配信は `web/src/app/serwist/[path]/route.ts`
  （`createSerwistRoute` が `sw.ts` を esbuild で束ね、`next build` 時に `/serwist/sw.js` として静的生成する。
  `.next/static/` の JS / CSS の一覧が precache として埋め込まれる）。登録は `web/src/components/service-worker-provider.tsx`。
- キャッシュするのはハッシュ付きの `/_next/static/*`（precache の JS / CSS と、使われたときに残すフォント・画像）だけ。
  ページ（HTML / RSC）・Better Auth（`/api/auth/*`）・Rust API（`/local/api/*`）は Service Worker が関与しないので、
  認証付きのレスポンスが Cache Storage に残ることはない。オフライン用のフォールバックページは無い。
- `next dev` では登録しない（同じオリジンに残っている登録とキャッシュも消す）。挙動を確かめるときは本番ビルドで:
  `pnpm build && pnpm start -p 3100` → DevTools の Application タブ（Manifest / Service workers / Cache storage）。
  Lighthouse の PWA カテゴリは v12 で廃止されたため、`pnpm dlx lighthouse@11 http://localhost:3100/ --only-categories=pwa` で見る。
- `/serwist/*` には `next.config.ts` の `headers()` で `Cache-Control: no-cache` を付けている
  （静的生成されたルートに Next が付ける `s-maxage=31536000` のままだと、CDN に古い `sw.js` が残ってデプロイ後も更新されない）。

## 開発

前提: Node 22+ / pnpm / Rust (rustup) / ローカルの PostgreSQL

### web

```sh
cd web
pnpm install

# 環境変数 (値は .env.example のコメント参照)
cp .env.example .env.local

# Better Auth 用のテーブルを作成
# (@better-auth/cli はランタイムより古いスキーマを生成するため使わないこと)
createdb discalendar_dev
pnpm db:migrate

pnpm dev
```

### api

web と同じ DB を使う（Better Auth のセッションを API 側で検証するため）。
サーバー選択以降の画面は API が動いていないと表示できない。

```sh
cd api
cp .env.example .env   # DATABASE_URL / DISCORD_BOT_TOKEN / BETTER_AUTH_SECRET (web と同じ値)
cargo run              # 起動時にマイグレーション適用、http://127.0.0.1:8080 (Swagger UI: /docs/)
```

http://localhost:3000 でランディング、`/login` から Discord ログイン、
`/dashboard` でサーバー選択 → 各サーバーのカレンダーが開く。
LP の「BOT を導入する」は `/invite` (Route Handler) が `DISCORD_BOT_INVITE_URL` (未設定なら `DISCORD_CLIENT_ID` から組み立て) へリダイレクトする。
サイト名・説明・公開 URL・外部リンクなどの定数は `web/src/lib/site.ts` にまとめてある。

Discord ログインを通すには [Discord Developer Portal](https://discord.com/developers/applications) のアプリで
OAuth2 Redirects に `http://localhost:3000/api/auth/callback/discord` を登録し、
Client Secret を `.env.local` の `DISCORD_CLIENT_SECRET` に設定する。

サーバー選択画面の「Bot が参加しているサーバー」は API が `guilds` テーブル（Bot が書き込む）で判定する。
ローカルでは下の bot を起動すれば参加中のサーバーが登録される。Bot を動かさない場合は手で登録しておく:

```sh
psql -d discalendar_dev -c "INSERT INTO guilds (guild_id, name, avatar_url, locale) VALUES ('<guild_id>', '<name>', NULL, 'ja') ON CONFLICT DO NOTHING"
```

### 管理コンソール (/admin)

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

### bot

api と同じ DB と Bot トークンを使う。起動すると Discord に接続し、参加中のサーバーを `guilds` テーブルに反映する
（Bot をサーバーに招待・退出させるとテーブルが更新される）。
スラッシュコマンド (`/help` `/create` `/list` `/init` `/invite`) は、Bot のオーナーがテスト用サーバーで
`@DisCalendar register` と送って「Register in guild」を押すと使えるようになる（詳細は [bot/README.md](bot/README.md)）。

```sh
cd bot
cp .env.example .env   # DATABASE_URL / DISCORD_BOT_TOKEN (api と同じ値)、BOT_LOG_CHANNEL_ID は任意
cargo run              # マイグレーションは api が適用するので、先に api を一度起動しておく
```

### テスト

web のユニットテスト (Vitest) と E2E (Playwright) がある。api / bot のテストは `cargo test --workspace` (Postgres が必要、[api/README.md](api/README.md))。

```sh
cd web
pnpm test     # Vitest: src/**/*.test.ts (純粋ロジック: 予定フォームのスキーマ・API との変換・終日予定の 1 日ずらし・エラー整形)
pnpm e2e      # Playwright: e2e/*.spec.ts (初回は pnpm exec playwright install chromium でブラウザを入れる)
pnpm shot     # 同じ環境で LP / 使い方のスクリーンショット (src/assets/) を撮り直す (後述)
```

E2E は api (Rust) + Postgres + Next.js を自動で立ち上げ、ログイン → サーバー選択 → 予定の作成・編集・ドラッグ移動 (とロールバック)・削除 →
サーバー設定 (restricted) の切替と非管理者の表示 を通す。Discord には繋がない:

- Discord OAuth は通さず、Better Auth の `user` / `session` / `account` 行を DB に直接作って署名付き cookie をブラウザに入れる (`e2e/seed.ts`)
- Discord API は `e2e/discord-mock.ts` (固定のユーザー・ギルド・権限、`e2e/fixtures.ts`) に差し替える。
  web と api は環境変数 `DISCORD_API_BASE_URL` でこのモックに向く (未設定なら本物の Discord)
- ポートは dev とぶつからない web 3100 / api 8180 / モック 8190 (`E2E_WEB_PORT` などで変更可)。dev サーバーを動かしたままでも実行できる
- DB は `E2E_DATABASE_URL` (未設定なら `web/.env.local` の `DATABASE_URL` の DB 名を `discalendar_e2e` に変えたもの) を使い、
  無ければ作る。**開始時に中身を消す**ので、DB 名に `e2e` を含まないと実行を拒否する
- api は `cargo run -p discalendar-api` (初回はビルドに時間がかかる)。ビルド済みのバイナリを使うなら `E2E_API_COMMAND=./target/debug/discalendar-api`。
  web はローカルでは `next dev`、CI (`CI=true`) では `next build` + `next start`
- 失敗時のスクリーンショットは `web/test-results/`、CI では `playwright-report` アーティファクト (`pnpm exec playwright show-report` で見られる)

設定は [web/playwright.config.ts](web/playwright.config.ts) と [web/vitest.config.mts](web/vitest.config.mts)。

#### スクリーンショットの撮り直し (`pnpm shot`)

LP と使い方に貼っている画像 (`web/src/assets/lp/`, `web/src/assets/docs/`) は、上と同じ E2E 環境で撮り直す。

```sh
cd web
pnpm shot     # 7 枚を src/assets/ に上書きする (e2e/screenshots/shots.spec.ts)
```

`E2E_SCREENSHOT=1` のときだけ、ユーザー名とサーバー名が画像に載せてよいもの (`ゲーム部` など) に差し替わり、
Discord モックのポートが 8191 になり (`revalidate` のキャッシュを通常のテストと混ぜないため)、ブラウザの言語が日本語になる。
撮影用のテストは通常の `pnpm e2e` と CI では動かない。web / api は使い回さず必ず起動し直すので、
前のテストのサーバーが残っていたら止めてから実行する。

対象画像の一覧・出来上がりの確認観点・詰まったときの対処は `.agents/skills/update-screenshots/` にまとめてある。

### Docker (compose) で動かす

ルートの `compose.yaml` で db (postgres:18) / api / web / bot をまとめて動かせる。各イメージは
`web/Dockerfile` (Next.js standalone) / `api/Dockerfile` / `bot/Dockerfile` から作る。ステージング (#26) や本番 (#12) でも
同じ compose を使い、イメージは GHCR (`ghcr.io/shirataki2/discalendar-{web,api,bot}`) から pull する想定。

```sh
cp .env.example .env          # POSTGRES_PASSWORD / BETTER_AUTH_SECRET / DISCORD_* を設定 (コメント参照)
docker compose build          # web は pnpm install + next build、api / bot は cargo build --release (初回は時間がかかる)
docker compose up -d          # db → api → web の順に起動。http://localhost:3000 (WEB_PORT で変更。BETTER_AUTH_URL は未設定ならこの URL に追従する。
                              # 既定では 127.0.0.1 にだけ bind。LAN に見せるなら WEB_BIND=0.0.0.0)
docker compose logs -f web api
```

- マイグレーション: api は起動時に `api/migrations/` を適用する。web は `AUTO_MIGRATE=true` (compose / Dockerfile の既定) のとき
  起動時に Better Auth のテーブルを作成・更新する (`web/src/instrumentation.ts`。ローカル開発の `pnpm db:migrate` と同じ内容)
- web の `/local/api/*` → api の rewrites の宛先は **ビルド時**に決まる (`web/Dockerfile` の `API_URL`、既定 `http://api:8080`)。
  compose のサービス名 `api` を変えるときは `--build-arg API_URL=...` でビルドし直す。api のポートはホストに公開しない
  (必要なら `compose.override.yaml` で `ports` を足す)
- bot は既定では起動しない。`docker compose --profile bot up -d` で起動する。**同じトークンの Bot が他で動いていると通知が
  二重に届く**ので、ローカルではテスト用 Discord アプリのトークンを使うこと (旧 Bot との入れ替え手順は #12)
- DB は compose 内のボリューム `db-data` に保存される。既存の DB を使う場合は各サービスの `DATABASE_URL` を override する

### staging への自動デプロイ

`main` にマージされると `.github/workflows/deploy-staging.yml` が web / api / bot のイメージを GHCR
(`ghcr.io/shirataki2/discalendar-{web,api,bot}`、タグ `sha-<short sha>` と `staging`) に push し、Tailnet 内の staging ホストに
ssh して `docker compose pull && up -d` する (<https://staging.discalendar.app>)。設計の経緯と選択肢は #26。

- **ロールバック**: Actions の "Deploy staging" → "Run workflow" で `image_tag` に過去の `sha-xxxxxxx` を指定する (ビルドは飛ばして deploy だけ行う)。
  手動実行も `main` 以外の ref では動かない (ワークフローの `if`)。Environment `staging` の Deployment branches も `main` だけに制限しておく
- **ホスト側の準備** (手作業。`/opt/discalendar-staging` を Repository variable `STAGING_COMPOSE_DIR` で変更可):
  `compose.yaml` (デプロイのたびに上書き配布される) と `.env` (`.env.example` を元に staging の値。`COMPOSE_PROFILES=bot,tunnel`、
  `IMAGE_TAG` はデプロイが書き換える) を置き、`docker login ghcr.io` しておく (パッケージを public にしていれば不要)。
  staging 用に別の Discord アプリ (Bot トークン / Client ID / Secret) を使い、Redirects に `https://staging.discalendar.app/api/auth/callback/discord` を登録する。
  DB は compose 内の `db-data` ボリューム。公開は compose の `cloudflared` (Cloudflare Zero Trust で Tunnel を作り、Public Hostname を
  `http://web:3000` に向けてトークンを `.env` の `TUNNEL_TOKEN` に入れる)
- **GitHub 側の設定** (Environment `staging`): secrets `TS_OAUTH_CLIENT_ID` / `TS_OAUTH_SECRET` (Tailscale の OAuth クライアント。scope `auth_keys`、
  tag `tag:ci`。ACL の `tagOwners` に `tag:ci` を足し、`tag:ci` からホストの ssh ポートへの接続を許可する)、`STAGING_SSH_HOST` / `STAGING_SSH_USER`、
  `STAGING_SSH_KEY` (鍵認証のとき。Tailscale SSH を使うなら不要)。ssh のポートが 22 以外なら variable `STAGING_SSH_PORT`
  (deploy ジョブは Environment に属するので Environment `staging` / Repository どちらの Variables でもよい)。
  Repository variables (Environment ではなくリポジトリの Variables。build ジョブは Environment に属さないため): `STAGING_PLATFORMS`
  (ホストが arm64 なら `linux/arm64`)、`STAGING_BUILD_RUNNER` (arm64 なら `ubuntu-24.04-arm`。QEMU でもビルドできるが Rust が極端に遅い)、
  `STAGING_COMPOSE_DIR` (任意)、`STAGING_SITE_URL` (任意。既定 `https://staging.discalendar.app`)
- **web は 2 本ビルドされる** (#87): OGP などの絶対 URL は `next build` 時に焼き込まれるため、本番へそのまま配る既定のビルド
  (本番ドメイン。`sha-xxxxxxx`) と、staging のドメイン (`STAGING_SITE_URL`) を焼き込んだビルド (`sha-xxxxxxx-staging`) を作る。
  staging ホストは後者を使う (`.env` の `WEB_IMAGE_TAG` をデプロイが書き換える)。`-staging` が無いタグへロールバックしたときは
  既定のビルドに落ちる (OGP だけ本番の URL になる)
- ホストで動く手順は `.github/scripts/deploy.sh` (healthy になるまで待ち、失敗したらログを出して exit 1。本番と共通)

### 本番 (discalendar.app) へのデプロイ

本番は staging と同じ仕組みで、`.github/workflows/deploy-production.yml` を手動実行 (`workflow_dispatch`) して反映する。
ビルドはせず、staging へのデプロイ時に GHCR へ push 済みの `sha-<short sha>` タグをそのまま使う (`image_tag` が必須入力。
**staging で動作確認したタグだけを指定する**。実行前に 3 イメージとも GHCR にあるか検証する)。旧版からの切替手順は #12。

- **デプロイ / ロールバック**: Actions の "Deploy production" → "Run workflow" で `image_tag` に `v3.x.y`
  (リリースタグ。下記「リリース」で `sha-*` と同じ digest に付け直したもの) か `sha-xxxxxxx` を指定する。
  ロールバックも同じ手順で過去のタグを指定するだけ。ただし**戻す先より後に DB マイグレーションが入っている場合は、
  イメージを戻す前に DB も戻す** (下記「マイグレーションが入った版から戻す」)
- **staging と同居する**: 本番ホストは staging と同一マシンのため、compose のプロジェクト名とポートを `.env` で分ける。
  `compose.yaml` の `name: discalendar` は staging が使っているので、本番の `.env` には **`COMPOSE_PROJECT_NAME=discalendar-prod`**
  (compose ファイルの `name:` より優先される) と、staging (3000) と重ならない **`WEB_PORT`** (例: 3001) を入れる。
  コンテナは `discalendar-prod-*`、DB ボリュームは `discalendar-prod_db-data` になる
- **ホスト側の準備** (手作業。`/opt/discalendar` を variable `PRODUCTION_COMPOSE_DIR` で変更可): staging と同様に
  `compose.yaml` (デプロイのたびに上書き配布される) と `.env` (`.env.example` を元に本番の値) を置く。`.env` は上記の
  `COMPOSE_PROJECT_NAME` / `WEB_PORT` のほか、`BETTER_AUTH_URL=https://discalendar.app`、本番 Discord アプリの `DISCORD_*`
  (Redirects に `https://discalendar.app/api/auth/callback/discord` を登録)。`COMPOSE_PROFILES` は**旧版と入れ替えが終わるまで
  `tunnel` だけにして bot を外す** (旧 Bot と同時に動くと通知が二重に届く。入れ替え手順は #12)。
  公開は staging と同じく Cloudflare Tunnel (本番用の Tunnel を作り、Public Hostname `discalendar.app` → `http://web:3000`、
  トークンを `.env` の `TUNNEL_TOKEN` へ)。`www.discalendar.app` は Tunnel では受けず、Cloudflare の Redirect Rule で apex へ 301 させる
- **GitHub 側の設定** (Environment `production`): secrets `TS_OAUTH_CLIENT_ID` / `TS_OAUTH_SECRET` (staging と同じ値でよい)、
  `PRODUCTION_SSH_HOST` / `PRODUCTION_SSH_USER`、`PRODUCTION_SSH_KEY` (Tailscale SSH を使うなら不要)。variables
  `PRODUCTION_SSH_PORT` (既定 22) / `PRODUCTION_COMPOSE_DIR` (任意)。Deployment branches を `main` だけに制限し、
  required reviewers を付けて誤ったデプロイを防ぐ

### リリース (バージョンと GitHub のタグ)

web / api / bot は 1 つのバージョンを共有し (`web/package.json` / `api/Cargo.toml` / `bot/Cargo.toml` / `Cargo.lock` の 4 か所。
CI の `version` ジョブが `.github/scripts/check-versions.sh` でずれを弾く)、リリースのたびに揃えて上げる。
#12 の本番切替を **v3.0.0** とし、以降は semver (機能追加なら minor、不具合修正なら patch) で上げる。
手順とその判断基準は `.agents/skills/release/` (Claude Code / Codex 共通スキル) にまとめてある。

リリースの実体は git のタグ `v3.x.y` で、push すると `.github/workflows/release.yml` が動く:

1. タグの形式・4 か所のバージョンとの一致・タグのコミットが `main` にあることを確認する
2. staging へのデプロイで GHCR に push 済みの `sha-<short sha>` イメージ 3 つに、**同じ digest のまま**
   `v3.x.y` (プレリリースでなければ `latest` も) を付け直す (`docker buildx imagetools create`。再ビルドしないので
   staging で検証したものと必ず同一。イメージが未ビルドならエラーで止まるので "Deploy staging" の完了を待ってタグを打つ)
3. 更新履歴 (`web/src/content/changelog.mdx`) の前回タグ以降のエントリからリリースノートを作り、GitHub Release を公開する

```sh
# main にマージ済みの状態から
.claude/skills/release/scripts/bump-version.sh minor   # 4 か所を書き換える → PR にしてマージ
git switch main && git pull --ff-only
git tag -a v3.1.0 -m "v3.1.0" && git push origin v3.1.0
```

**本番への反映は自動にしていない**。タグを打った後で Actions の "Deploy production" を `image_tag: v3.1.0` で実行する
(Environment `production` の承認を通す運用をそのまま残すため)。ロールバックも同じ画面で前の版のタグを指定する。
`/admin` の「api のバージョン」はこの版が出るが、「イメージタグ」は実行ファイルに焼き込まれた `sha-xxxxxxx` のまま (#37)。

- `latest` は**最新の正式リリース**を指す (`compose.yaml` の `IMAGE_TAG` 既定値)。`release` ジョブの後に動く `latest` ジョブが、
  公開済みの GitHub Release のうち一番新しいものを見て**そのリリースのイメージ**に向ける (自分のタグが最新でなくても
  そこに収束する)。プレリリースと下書きは候補に入らないので、古いタグの再実行や打ち間違えたタグが残っていても
  巻き戻ったり止まったりしない
- ⚠️ **`v*` タグの作成を制限するルールセットを入れておくこと** (Settings → Rules → Rulesets → Tag ruleset で
  `v*` を対象に "Restrict creations" + Bypass list にリリース担当者)。タグから起動するワークフローは
  **そのタグのコミットにある `release.yml` がそのまま動く**ため、main に入っていない改変版のワークフローを
  タグ付きで push されると、`contents: write` / `packages: write` のトークンで Release や GHCR タグを操作できてしまう
  (ワークフロー内の「main のコミットか」の確認も、その改変版では消せる)。ブランチ保護だけでは塞げない

### エラー監視 (Sentry) とコンテナログ

方針 (#17): エラー追跡は Sentry SaaS (Developer 無料枠) を web / api / bot の 3 サービスに入れ、コンテナログは
compose の logging 設定 (json-file、10MB × 3 世代) でローテーションだけ行う。ログの横断検索・ログベースのアラートは #104。
DSN が未設定なら 3 サービスとも何も送らない (ローカル開発・CI・E2E はそのまま)。

- **Sentry 側の準備**: プロジェクトを web (platform: Next.js) / api / bot (platform: Rust) の 3 つ作り、それぞれの DSN を控える。
  通知は Sentry 側の設定だけで足りる (コードは不要)。Discord インテグレーション (Settings → Integrations → Discord) を
  使えればサポートサーバーのチャンネルへ流せるが、無料プランで third-party integrations が使えるかは
  契約時点の Sentry のプラン次第なので、使えなければメール通知にする
- **api / bot**: DSN は実行時の環境変数。ホストの `.env` に `API_SENTRY_DSN` / `BOT_SENTRY_DSN` を入れる
  (staging ホストは `SENTRY_ENVIRONMENT=staging` も)。同種エラーの嵐で無料枠 (5,000 件/月) が溶けそうなときは
  `.env` の `SENTRY_SAMPLE_RATE` (0.0〜1.0。compose が両サービスへ渡す) で送信率を絞り、`docker compose up -d` で反映する。
  起動に失敗してプロセスが終わるとき (設定の誤り・DB 接続不可など) もイベントとして送る
- **web**: ブラウザに配る DSN なので `next build` 時に焼き込まれる。GHCR のイメージには Repository variable
  `WEB_SENTRY_DSN` をデプロイ CI (`deploy-staging.yml`) が build-arg で渡す (未設定なら Sentry 無効のままビルドされる)。
  environment タグは焼き込まれた `NEXT_PUBLIC_SITE_URL` から導出する (本番ドメイン → production、`staging.` → staging)。
  ブラウザのスタックトレースをソースマップで戻したい場合は、Repository variables `SENTRY_ORG` / `SENTRY_PROJECT_WEB` と
  secret `SENTRY_AUTH_TOKEN` を設定する (未設定ならアップロードはスキップ)。トークンは Sentry の組織設定
  (Settings → Auth Tokens) で作る **Organization Auth Token** (`sntrys_` で始まる) を使う。CI 向けに権限が固定されていて
  scope を選ぶ必要がなく、個人アカウントに紐づかないので発行者の権限が変わってもビルドが壊れない
  (個人トークンでも動くが、その場合は scope に `project:releases` が要る)。
  Organization Auth Token を使う場合も `SENTRY_ORG` / `SENTRY_PROJECT_WEB` の指定は必要
- **同じ障害を二重に数えない**: api の 5xx は `ApiError::error_response` のログだけをイベントにし (`sentry-actix` の
  `capture_server_errors` は無効。ミドルウェアはリクエスト情報の付与のために残している)、bot のコマンドの panic は
  panic 側だけをイベントにする (poise が拾ったあとのログはパンくず扱い)。どちらも無料枠を余分に消費しないため
- **リリースとの紐付け**: api / bot はクレートのバージョン (`discalendar-api@3.x.y` など) を release として送る。
  バージョンは 3 サービス共通 (上記「リリース」) なので、どの版で出たエラーかは release タグで追える

### DB のバックアップと復元 (compose)

compose の db (postgres:18) はボリューム (`<プロジェクト名>_db-data`) に保存される。その環境の compose ディレクトリで実行する
(本番は `.env` の `COMPOSE_PROJECT_NAME` が効くのでコマンドは共通):

```sh
# バックアップ (カスタム形式)。定期実行し、ホスト外にもコピーしておく
# (-T は TTY 割り当てを止めるオプション。付けないとバイナリ出力が壊れることがある)
docker compose exec -T db pg_dump -U discalendar -d discalendar -Fc > discalendar_$(date +%Y%m%d%H%M%S).dump

# 復元は空の DB に対して行う。先に db だけ起動して復元する
# (api を先に起動すると起動時マイグレーションが空 DB に走り、復元と衝突する)
docker compose up -d db
docker compose exec -T db pg_restore -U discalendar -d discalendar --no-owner --no-privileges < <ダンプファイル>
docker compose up -d
```

- 旧版 (postgres 13、オーナー `postgres`) からの移行では `--no-owner --no-privileges` が必須。旧 DB 側では
  `pg_dump -Fc --no-owner --no-privileges -U postgres <DB名>` で取得する (切替手順の全体は #12)
- staging に本番データを入れて検証するときも同じ手順で復元する (その間は bot プロファイルを外す)

### DB を書き換えるマイグレーションを適用するとき

既存の行を書き換えるマイグレーション (カラムの型変換など) は、適用中その表への書き込みが止まる。
**Bot の通知タスクは起動時に直近 5 分ぶんしか遡らない** (`bot/src/tasks/notify.rs` の `STARTUP_LOOKBACK`。
それより古い分は「陳腐化した通知」として送らずに早送りする) ので、デプロイの停止時間が 5 分を超えると、
その間に発火するはずだった通知は送られないまま終わる。

適用前に対象テーブルの規模を確かめる:

```sh
docker compose exec -T db psql -U discalendar -d discalendar -c "SELECT count(*) FROM events"
```

数万行なら型変換は一瞬で終わる。桁が違うようなら、サポートサーバーでの告知や、
予定の少ない時間帯での実施を検討する。

### マイグレーションが入った版から戻す

イメージだけ戻しても DB は戻らない。**戻す先より後に入ったマイグレーションがあるときは、DB を先に戻す**:

- 古い api / bot は変更後のスキーマを読めない (例: `notifications` を `TEXT[]` としてデコードするので、
  JSONB のままだと予定のクエリが失敗し続ける)
- `sqlx::migrate!` は既定 (`ignore_missing = false`) なので、**自分の `migrations/` に無いバージョンが
  `_sqlx_migrations` にあるだけで api の起動が失敗する**

戻すための SQL は `api/rollback/<マイグレーションと同じ version>_*.sql` に置いてある (`migrations/` ではないので自動実行はされない)。
ファイル冒頭に手順があり、要点は「api / bot を止める → ダンプを取る → SQL を流す → 前の版のタグでデプロイ」。
DB を変えるマイグレーションを追加する PR では、対になる戻し方をここに用意する。

## 開発の進め方 (GitHub)

- 作業は Issue に登録する (`.github/ISSUE_TEMPLATE/` のフォーム: 不具合報告 / 機能要望 / 開発タスク)。
  マイルストーン「v3 リリース」と `area:*` ラベルを付けて進捗を追う
- `main` への直接 push は禁止 (ルールセット)。ブランチを切って PR を作り、本文の `Closes #N` で Issue と紐付ける
  (`gh issue develop N --checkout` でブランチを作れる)。マージは squash のみで、マージ後のブランチは自動削除される
- PR では CI (`.github/workflows/ci.yml`: web は Biome / tsc / Vitest / next build、rust (api / bot) は rustfmt / clippy / test、
  e2e は Playwright (web か rust に変更があるとき)) が通ることが必須
- AI レビュー: Claude (`.github/workflows/claude-code-review.yml`、secret `CLAUDE_CODE_OAUTH_TOKEN` が必要) と
  Codex (Codex クラウドの GitHub 連携で自動レビュー) が PR を確認する。コメントで `@claude` / `@codex review` と呼ぶと追加で依頼できる。
  レビューの観点は [AGENTS.md](AGENTS.md) の「Code Review Rules」
- 依存の更新は Dependabot (`.github/dependabot.yml`) が毎週まとめて PR を出す

### AI エージェントの環境を整える

#### Claude Code のスキルを Codex でも使う

スキルの詳細手順とスクリプトの正本は `.claude/skills/` に置き、Codex がリポジトリスキルとして検出する
`.agents/skills/` から正本を読む。片方だけを直して手順が分岐しない構成にしている。

- Codex CLI / IDE では `/skills` または `$issue-driven-dev` のように指定する。依頼が description に合えば自動選択もされる
- Claude Code では従来どおり `.claude/skills/` が使われる
- 対応するスキルは `issue-driven-dev` / `cleanup-workspace` / `release` / `update-screenshots`

Codex のスキル探索場所と形式は [Build skills](https://developers.openai.com/codex/skills) を参照。

#### Codex のローカル worktree

Codex デスクトップアプリで新しいタスクを作るときに **Worktree** を選ぶと、アプリ管理の隔離された checkout で並行作業できる。

- `.worktreeinclude` が、Git 管理外の `.env` / `web/.env.local` / `api/.env` / `bot/.env` を存在するときだけ managed worktree へコピーする。
  中身が Git に追加されるわけではない。実トークンを含むため、コピー先も削除時まで秘密情報として扱う
- Codex の Settings → Local environments で、このリポジトリ用の Setup script に次を設定する

  ```bash
  bash .agents/scripts/setup-worktree-environment.sh
  ```

- 作業を残すときは **Create branch here** で `codex/issue-<N>-<slug>` を作るか、**Handoff** で Local に移す
- Claude Code や CLI から手動で worktree を作る場合は `issue-driven-dev` スキルの `setup-worktree.sh` を使う

managed worktree と `.worktreeinclude` の仕様は [Git worktrees](https://developers.openai.com/codex/app/worktrees) を参照。

#### Codex / Claude Code のクラウド環境

両環境ともリポジトリを隔離環境へ clone するため、Git 管理外の旧実装 `tmp/DisCalendarV2/` とローカルの `.env` は持ち込まれない。
CI 相当の検証に必要な PostgreSQL、マイグレーション、Node / Rust 依存、ダミー `.env` は
`.agents/scripts/setup-cloud-environment.sh` で共通に準備する。

Environment variables には秘密情報ではなく、次のダミー値を設定する:

```dotenv
SQLX_OFFLINE=true
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/discalendar_dev
BETTER_AUTH_SECRET=cloud-dummy-secret-not-used-at-runtime
BETTER_AUTH_URL=http://localhost:3000
```

Codex cloud は GitHub を接続して Environment を作り、次を設定する。Setup script の環境はキャッシュされ、Maintenance script は
キャッシュ再開後に選択ブランチへ checkout してから依存と DB の状態を更新する。Environment の登録自体はアカウント側の設定なので、
リポジトリを clone しただけでは作成されない。

```bash
# Setup script
bash .agents/scripts/setup-cloud-environment.sh --install-tools

# Maintenance script
bash .agents/scripts/setup-cloud-environment.sh
```

Agent internet access は通常の実装に不要なら既定の無効のままにする (Setup script 中の依存取得にはネットワークを使える)。
設定方法は [Codex cloud](https://developers.openai.com/codex/cloud) と
[Cloud environments](https://developers.openai.com/codex/cloud/environments) を参照。

Claude Code では `.claude/settings.json` の SessionStart hook が、`CLAUDE_CODE_REMOTE=true` のときだけ同じ共通スクリプトを呼ぶ。
初回の toolchain / sqlx-cli 取得には Environment の Setup script として `bash .agents/scripts/setup-cloud-environment.sh --install-tools` を設定する。
GitHub App の接続など Claude 固有の設定は [Configure cloud environments](https://code.claude.com/docs/en/cloud-environments) /
[Claude Code on the web](https://code.claude.com/docs/en/claude-code-on-the-web) を参照。

最初のセッションでは `rustup --version` / `sqlx --version` / `pnpm -v` / `psql --version` を確認し、`[agent-setup]` ログで
PostgreSQL のマイグレーションと `pnpm install` が完了したことを確かめる。クラウドには実トークンを入れないため、Discord ログイン、
Bot の実機確認、対話ブラウザでの見た目確認はローカルへ引き継ぐ。clone 内に追加の worktree は作らない。

## ルート構成 (web)

| パス | 内容 |
|---|---|
| `/` | ランディング |
| `/login` | Discord ログイン |
| `/dashboard` | サーバー選択（Bot 参加済み / 招待可能なサーバー） |
| `/dashboard/[id]` | ギルドごとのカレンダー |
| `/admin` | 管理コンソール（`ADMIN_DISCORD_USER_IDS` のユーザーのみ。それ以外は 404）: 件数・DB / マイグレーション・ビルド情報の概要 |
| `/admin/guilds`, `/admin/guilds/[id]` | 管理コンソール: 全ギルドの一覧・検索と Discord との差分検出、ギルドごとの予定の閲覧・編集 |
| `/admin/sql` | 管理コンソール: 読み取り専用 SQL コンソール (結果の表・実行履歴) と定型操作 |
| `/admin/users` | 管理コンソール: ユーザーの検索とセッションの確認・強制ログアウト |
| `/admin/audit-logs` | 管理コンソール: 監査ログの閲覧 (操作の種類・実行者で絞り込み) |
| `/api/auth/*` | Better Auth（OAuth コールバック含む） |
| `/manifest.webmanifest`, `/serwist/sw.js` | PWA のマニフェストと Service Worker（`app/manifest.ts`, `app/sw.ts`） |
| `/local/api/*` | Rust API へのプロキシ（rewrites） |
