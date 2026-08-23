# DisCalendar

> <https://discalendar.app>

Discord 用のカレンダーアプリ。予定の作成から通知まで、ブラウザから面倒なコマンド操作なしで扱える。

このリポジトリは稼働中の DisCalendar（旧コード: `tmp/DisCalendarV2`、git 管理外）をモダナイズするプロジェクト。最終的に front / api / bot をこのリポジトリ配下に集約する。

## 構成

| ディレクトリ | 内容 | 状態 |
|---|---|---|
| `web/` | Next.js 16 (App Router) + React 19 + FullCalendar v7 + Better Auth + TanStack Query + shadcn/ui + React Hook Form / Zod | LP (静的生成、OGP / favicon / manifest)、使い方ページ (MDX、静的生成)、Discord ログイン、サーバー選択、カレンダー (予定の取得 / 作成・編集ダイアログ / 移動 / 削除)、サーバー設定ダイアログ (restricted モード) |
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

### web の使い方ページ (docs)

- `/docs/<slug>` は `@next/mdx` で静的生成する（旧 `@nuxt/content` の 7 ページと同じ URL）。本文は `web/src/content/docs/<slug>.mdx`、
  ページの一覧と並び順（サイドナビ・前後リンク・`generateStaticParams`）は `web/src/lib/docs.ts`、
  描画は `web/src/app/docs/[slug]/page.tsx`（`dynamicParams = false`）と `web/src/app/docs/layout.tsx`。
- MDX の見出し・リンクなどの見た目は `web/src/mdx-components.tsx`、本文用の部品（スクリーンショット、ボタン、注意書き、手順、コマンドカード）は
  `web/src/components/docs/`。表を使うため `remark-gfm` を入れている（Turbopack にはパッケージ名の文字列で渡す）。
- スクリーンショットは `web/src/assets/docs/`（`next/image` の静的 import）。LP と共用のものは `web/src/assets/lp/`。
  ページを増やすときは `.mdx` を足して `DOC_PAGES` に 1 行追加する。

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
ローカルで試すときは `api/.env` に自分の Discord ユーザー ID を入れて api を再起動し、`/dashboard` のヘッダーに出る
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
  `STAGING_COMPOSE_DIR` (任意)
- ホストで動く手順は `.github/scripts/deploy-staging.sh` (healthy になるまで待ち、失敗したらログを出して exit 1)

## 開発の進め方 (GitHub)

- 作業は Issue に登録する (`.github/ISSUE_TEMPLATE/` のフォーム: 不具合報告 / 機能要望 / 開発タスク)。
  マイルストーン「v3 リリース」と `area:*` ラベルを付けて進捗を追う
- `main` への直接 push は禁止 (ルールセット)。ブランチを切って PR を作り、本文の `Closes #N` で Issue と紐付ける
  (`gh issue develop N --checkout` でブランチを作れる)。マージは squash のみで、マージ後のブランチは自動削除される
- PR では CI (`.github/workflows/ci.yml`: web は Biome / tsc / next build、rust (api / bot) は rustfmt / clippy / test) が通ることが必須
- AI レビュー: Claude (`.github/workflows/claude-code-review.yml`、secret `CLAUDE_CODE_OAUTH_TOKEN` が必要) と
  Codex (Codex クラウドの GitHub 連携で自動レビュー) が PR を確認する。コメントで `@claude` / `@codex review` と呼ぶと追加で依頼できる。
  レビューの観点は [AGENTS.md](AGENTS.md) の「Code Review Rules」
- 依存の更新は Dependabot (`.github/dependabot.yml`) が毎週まとめて PR を出す

## ルート構成 (web)

| パス | 内容 |
|---|---|
| `/` | ランディング |
| `/login` | Discord ログイン |
| `/dashboard` | サーバー選択（Bot 参加済み / 招待可能なサーバー） |
| `/dashboard/[id]` | ギルドごとのカレンダー |
| `/admin` | 管理コンソール（`ADMIN_DISCORD_USER_IDS` のユーザーのみ。それ以外は 404） |
| `/admin/guilds`, `/admin/guilds/[id]` | 管理コンソール: 全ギルドの一覧・検索と、ギルドごとの予定の閲覧・編集 |
| `/api/auth/*` | Better Auth（OAuth コールバック含む） |
| `/local/api/*` | Rust API へのプロキシ（rewrites） |
