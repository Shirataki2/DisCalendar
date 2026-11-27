# デプロイと運用

[README に戻る](../README.md)

パスとコマンドは、特記がなければリポジトリルートを基準に記載する。

## Docker (compose) で動かす

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

## サポート用 Bot (support-bot)

サポートサーバーにだけ入る別の Discord アプリ (#423)。問い合わせの記録・未返信の通知・毎朝のまとめを行う。
機能と Discord 側の準備は [support-bot/README.md](../support-bot/README.md)。

- compose の `support-bot` サービス (`--profile support-bot`)。**本番だけで有効にする** (staging でも同じトークンで動かすと二重に記録・通知する。
  staging ではイメージを作るだけで起動しない)
- DB は同じ Postgres の別データベース `discalendar_support`。初回起動時に support-bot が作り、`support-bot/migrations/` を自分で当てる
  (api の起動や `_sqlx_migrations` とは独立)。api / bot とは同じボリュームに乗るので、バックアップも同じ timer で取る (下の「DB のバックアップと復元」)
- **本番で有効にする手順**: `.env` に `SUPPORT_BOT_DISCORD_TOKEN` / `SUPPORT_GUILD_ID` / `SUPPORT_RECORD_CHANNEL_IDS` /
  `SUPPORT_STAFF_ROLE_IDS` / `SUPPORT_STAFF_CHANNEL_ID` (任意で `SUPPORT_BOT_SENTRY_DSN`) を入れ、`COMPOSE_PROFILES` に `support-bot` を足して
  `docker compose up -d`。GHCR の `discalendar-support-bot` パッケージは最初の push で作られるので、他のイメージと同じく public になっているか確かめる
  (本番デプロイはログインせずに `docker manifest inspect` する)
- **support-bot が無い版 (v3.29.0 以前) へ戻すとき**は、先に `COMPOSE_PROFILES` から `support-bot` を外す (その版のイメージが無いので pull に失敗する)。
  本番デプロイのワークフローはイメージが無いと警告を出す
- **不具合対応の補助 (#426) を有効にする手順**: `.env` に `SUPPORT_ANTHROPIC_API_KEY` (Claude API のキー。Console で利用上限を設定する) と、
  `SUPPORT_GITHUB_TOKEN` (対象リポジトリだけ・**Issues: Read and write だけ**の fine-grained トークン。期限つき) / `SUPPORT_GITHUB_REPO` (`Shirataki2/DisCalendar` の形) を入れて
  `docker compose up -d support-bot`。片方だけでもよい (未設定の機能はボタンが無効になる)。モデルを変えるときは `SUPPORT_CLAUDE_MODEL`。
  スタッフが押したときだけ送り、送った記録 (本文なし) は `support_ai_requests` に残る。キー・トークンが漏れたら Console / GitHub で失効させて入れ替える。
  送信の件数は `docker compose exec -T db psql -U discalendar -d discalendar_support -c "SELECT date_trunc('day', created_at) AS day, kind, outcome, count(*), sum(input_tokens), sum(output_tokens) FROM support_ai_requests GROUP BY 1, 2, 3 ORDER BY 1 DESC"` で見る。
  詳細は [support-bot/README.md](../support-bot/README.md#不具合対応の補助-426)
- 記録をやめて消すときは、support-bot を止めてから `docker compose exec -T db psql -U discalendar -d postgres -c 'DROP DATABASE discalendar_support'`

### サポート記録を Claude から読む (MCP)

サポート用 Bot の記録は、手元の Claude Code から読み取り専用の MCP サーバー (`discalendar-support-mcp`、#425) で読む。
ツールの中身・Claude Code への登録・信頼できない本文の扱いは [support-bot/README.md](../support-bot/README.md#claude-から記録を読む-mcp425)。
本番ホストには常駐させず、使うときだけ Tailscale SSH のトンネルで db に繋ぐ (db のポートは公開しない)。

**読み取り専用のロール** `discalendar_support_mcp` は 1 つで両方の DB を読む。見せるのはサポート用 DB の
`support_threads` / `support_messages` / `support_reminders` と、本体 DB の `guilds` / `guild_config` / `event_settings` だけ
(予定の中身・ログインのセッション・Webhook の URL などは見せない)。作り方 (本番ホストの compose ディレクトリで。何度流しても同じ結果になる):

```sh
# 1. ロールと権限を作る (SQL は support-bot/sql/mcp-readonly-role.sql。本番ホストに無ければ手元から標準入力で流す)
docker compose exec -T db psql -U discalendar -d postgres \
  -v support_db=discalendar_support -v main_db=discalendar -f - < support-bot/sql/mcp-readonly-role.sql
# 2. パスワードを設定する (対話で 2 回入力。値はパスワードマネージャーに保存し、リポジトリ・Issue・シェルの履歴に残さない)
docker compose exec db psql -U discalendar -d postgres -c '\password discalendar_support_mcp'
```

- 手元からは `support-bot/scripts/mcp-tunnel.sh <ssh 先> /opt/discalendar 15432` でトンネルを張り、
  `postgres://discalendar_support_mcp:<パスワード>@127.0.0.1:15432/<DB 名>` で繋ぐ
- ロール自体にも `default_transaction_read_only = on`・`statement_timeout = 10s`・`CONNECTION LIMIT 4` を付けている
- DB を `pg_restore --no-privileges` で戻すと権限が消えるので、復元したら 1. を流し直す。サポート用 DB を作り直したとき
  (support-bot の初回起動前に流すと `discalendar_support` が無くて失敗する) も同じ
- 見せる表を増やすときは SQL に足して流し直す (`ALL TABLES` や `DEFAULT PRIVILEGES` は使っていないので、新しい表は自動では見えない)
- パスワードが漏れた疑いがあれば 2. で変えるか、`ALTER ROLE discalendar_support_mcp NOLOGIN` で止める。消すときは両方の DB で
  `DROP OWNED BY discalendar_support_mcp` (権限の取り消し) を実行してから `DROP ROLE discalendar_support_mcp`

## staging への自動デプロイ

`main` にマージされると `.github/workflows/deploy-staging.yml` が web / api / bot のイメージを GHCR
(`ghcr.io/shirataki2/discalendar-{web,api,bot}`、タグ `sha-<short sha>` と `staging`) に push し、Tailnet 内の staging ホストに
ssh して `docker compose pull && up -d` する (<https://staging.discalendar.app>)。設計の経緯と選択肢は #26。

- **ロールバック**: Actions の "Deploy staging" → "Run workflow" で `image_tag` に過去の `sha-xxxxxxx` を指定する (ビルドは飛ばして deploy だけ行う)。
  手動実行も `main` 以外の ref では動かない (ワークフローの `if`)。Environment `staging` の Deployment branches も `main` だけに制限しておく
- **ホスト側の準備** (手作業。`/opt/discalendar-staging` を Repository variable `STAGING_COMPOSE_DIR` で変更可):
  `compose.yaml` (デプロイのたびに上書き配布される) と `.env` (`.env.example` を元に staging の値。`COMPOSE_PROFILES=bot,tunnel`、
  `IMAGE_TAG` はデプロイが書き換える) を置き、`docker login ghcr.io` しておく (パッケージを public にしていれば不要)。
  staging 用に別の Discord アプリ (Bot トークン / Client ID / Secret) を使い、Redirects に `https://staging.discalendar.app/api/auth/callback/discord` を登録する。
  Discord の Activity を使うときは、そのアプリで [Activity の設定](discord-activity.md#discord-developer-portal-の設定) も行う。
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

## 本番 (discalendar.app) へのデプロイ

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

## リリース (バージョンと GitHub のタグ)

web / api / bot は 1 つのバージョンを共有し (`web/package.json` / `api/Cargo.toml` / `bot/Cargo.toml` / `Cargo.lock` の 4 か所。
CI の `version` ジョブが `.github/scripts/check-versions.sh` でずれを弾く)、リリースのたびに揃えて上げる。
#12 の本番切替を **v3.0.0** とし、以降は semver (機能追加なら minor、不具合修正なら patch) で上げる。
手順とその判断基準は `.agents/skills/release/` (Claude Code / Codex 共通スキル) にまとめてある。

リリースの実体は git のタグ `v3.x.y` で、push すると `.github/workflows/release.yml` が動く:

1. タグの形式・4 か所のバージョンとの一致・タグのコミットが `main` にあることを確認する
2. staging へのデプロイで GHCR に push 済みの `sha-<short sha>` イメージ 3 つに、**同じ digest のまま**
   `v3.x.y` (プレリリースでなければ `latest` も) を付け直す (`docker buildx imagetools create`。再ビルドしないので
   staging で検証したものと必ず同一。イメージが未ビルドならエラーで止まるので "Deploy staging" の完了を待ってタグを打つ)
3. 更新履歴 (`web/src/content/changelog.mdx`) のそのバージョンの節 (リリース準備時に `bump-version.sh` が挿入した
   バージョン見出しの下のエントリ) からリリースノートを作り、GitHub Release を公開する

```sh
# main にマージ済みの状態から
.claude/skills/release/scripts/bump-version.sh minor   # 4 か所の書き換え + changelog.d の集約・削除とバージョン見出しを挿入 → PR にしてマージ
git switch main && git pull --ff-only
git tag -a v3.1.0 -m "v3.1.0" && git push origin v3.1.0
```

**本番への反映は自動にしていない**。タグを打った後で Actions の "Deploy production" を `image_tag: v3.1.0` で実行する
(Environment `production` の承認を通す運用をそのまま残すため)。ロールバックも同じ画面で前の版のタグを指定する。
`/admin` の「api のバージョン」はこの版が出るが、「イメージタグ」は実行ファイルに焼き込まれた `sha-xxxxxxx` のまま (#37)。

**更新の告知 (#430)**: 本番に反映されて support-bot が新しい版で起動すると、更新履歴のその版の節をサポートサーバーの
「更新通知」チャンネル (`.env` の `SUPPORT_RELEASE_CHANNEL_ID`) に 1 回だけ投稿し、アナウンスチャンネルなら公開 (crosspost) する。
タグを打った時点では投稿しない (本番に出ていないため)。告知済みの版はサポート用 DB に持つので、デプロイの再実行・再起動・
前の版へのロールバック・プレリリースでは投稿しない。Webhook URL やトークンは使わない (投稿は support-bot のトークンで行う)。

- **有効にする手順**: サポートサーバーに「更新通知」チャンネルを作ってアナウンスチャンネルにし、support-bot に
  チャンネルを見る・メッセージを送信・埋め込みリンクの権限を与える。本番の `.env` に `SUPPORT_RELEASE_CHANNEL_ID=<チャンネル ID>` を入れて
  `docker compose up -d support-bot`。**そのとき動いている版がまだ告知されていなければ、その場で告知する**
  (告知したくなければ、先に記録を入れる。support-bot/README.md「更新の告知」)
- 投稿されなかったら `docker compose logs support-bot | grep -i announce` を見る。投稿の失敗 (権限不足など) なら記録は取り消してあるので、
  直してから `docker compose restart support-bot` で告知し直す
- 本文は support-bot のイメージにビルド時に埋め込んだ更新履歴から作る。版を上げるリリース準備 PR で `bump-version.sh` が
  その版の節も作るので、新しい版のイメージには必ずその版の節が入っている (節が無いのは利用者に見える変更が無い版だけ)

- `latest` は**最新の正式リリース**を指す (`compose.yaml` の `IMAGE_TAG` 既定値)。`release` ジョブの後に動く `latest` ジョブが、
  公開済みの GitHub Release のうち一番新しいものを見て**そのリリースのイメージ**に向ける (自分のタグが最新でなくても
  そこに収束する)。プレリリースと下書きは候補に入らないので、古いタグの再実行や打ち間違えたタグが残っていても
  巻き戻ったり止まったりしない
- ⚠️ **`v*` タグの作成を制限するルールセットを入れておくこと** (Settings → Rules → Rulesets → Tag ruleset で
  `v*` を対象に "Restrict creations" + Bypass list にリリース担当者)。タグから起動するワークフローは
  **そのタグのコミットにある `release.yml` がそのまま動く**ため、main に入っていない改変版のワークフローを
  タグ付きで push されると、`contents: write` / `packages: write` のトークンで Release や GHCR タグを操作できてしまう
  (ワークフロー内の「main のコミットか」の確認も、その改変版では消せる)。ブランチ保護だけでは塞げない

## エラー監視 (Sentry) とコンテナログ

方針 (#17): エラー追跡は Sentry SaaS (Developer 無料枠。5,000 件/月・保持 30 日・1 ユーザー) を web / api / bot の
3 サービスに入れ、コンテナログは compose の logging 設定 (json-file、10MB × 3 世代) でローテーションする。
そこから溢れたログの横断検索と、ログベースの Discord 通知は下の「ログ集約 (Grafana Cloud)」が担う (#104)。
DSN が未設定なら 3 サービスとも何も送らない (ローカル開発・CI・E2E はそのまま)。

- **Sentry 側の準備**: プロジェクトを web (platform: Next.js) / api / bot (platform: Rust) の 3 つ作り、それぞれの DSN を控える
  (サポート用 Bot を動かすなら support-bot (Rust) も作り、DSN を `.env` の `SUPPORT_BOT_SENTRY_DSN` に入れる)。
  通知は Sentry 側の設定だけで足りる (コードは不要)。**無料プラン (Developer) の通知はメールのみ**で、
  Discord などの third-party integrations は Team プラン以上でないと使えない (2026-08 時点。実際に確認済み)。
  Discord のチャンネルへ流したい場合は、Team プランにするか、下のログ側のアラート (Grafana Cloud は
  無料枠で Discord 通知に対応) で代替する
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
  `capture_server_errors` と `tracing-actix-web` の `emit_event_on_error` はどちらも無効。ミドルウェアは
  リクエスト情報の付与のために残している)、bot のコマンドの panic は
  panic 側だけをイベントにする (poise が拾ったあとのログはパンくず扱い)。どちらも無料枠を余分に消費しないため
- **リリースとの紐付け**: api / bot はクレートのバージョン (`discalendar-api@3.x.y` など) を release として送る。
  バージョンは 3 サービス共通 (上記「リリース」) なので、どの版で出たエラーかは release タグで追える

## ログ集約 (Grafana Cloud)

方針 (#104): compose の各サービスのコンテナログを Grafana Alloy (compose の `alloy`、`--profile logging`) が
Grafana Cloud の Loki に送り、横断検索とログベースのアラート (Discord 通知) をそこで行う。
Sentry が例外の中身を、こちらがログの流れと通知を受け持つ。

- **ラベル**: `env` (production / staging)・`service` (compose のサービス名)・`level` (api / bot / support-bot だけ)。
  api / bot / support-bot は `LOG_FORMAT=json` (compose の既定) で `tracing` を JSON 出力にしてあり、そこから `level` を起こす。
  ローカルで `docker compose logs` を人が読むときは `.env` に `LOG_FORMAT=text` を入れる
- **ホスト側**: `.env` に `GRAFANA_CLOUD_LOKI_URL` / `_USER` / `_TOKEN` を入れ、`COMPOSE_PROFILES` に `logging` を足す。
  設定ファイル (`infra/alloy/config.alloy`) はデプロイのたびに compose.yaml と一緒に配られる
- **リクエストごとの 1 行**: api はリクエストが終わるたびに `request completed` の行 (ステータス・メソッド・
  ルート・所要時間) を INFO で出す (#110)。リクエスト単位のエラー率とレイテンシはこれを `| json` で集計する。
  10 秒おきに叩かれる `/healthz` は DEBUG なので本番のログには出ない
- **アラート**: 「api / bot / support-bot の ERROR が 5 分で 10 件超」と「本番のログが 15 分途絶」の 2 本を Terraform
  (`infra/terraform/grafana/`) で管理し、Discord の Webhook に流す
- 準備の手順 (スタック・トークン・Terraform の apply) と LogQL の例は [infra/README.md](../infra/README.md) にまとめてある

## DB のバックアップと復元

compose の db (postgres:18) はボリューム (`<プロジェクト名>_db-data`) に保存される。

**日次バックアップは自動で走る** (#102)。本番ホストの systemd timer が毎日 JST 04:00 に `pg_dump -Fc` して
Cloudflare R2 (`s3://discalendar-backups/production/`) にアップロードする。30 日を過ぎた分は R2 の
ライフサイクルルールで自動削除され、直近 7 日ぶんはバケットロックで削除・上書きできない
(アップロード用のキーが漏れても消せない)。設置・確認・一覧の手順は [infra/README.md](../infra/README.md)、
バケットの定義は [infra/terraform/](../infra/terraform/)。
サポート用 Bot のデータベース (`discalendar_support`) があれば、同じ timer で `discalendar_support-<時刻>.dump` として別ファイルに上げる
(`backup.env` の `BACKUP_EXTRA_DB_NAMES`。スクリプトを更新したら本番ホストの `/opt/discalendar-backup/` に配り直す。手順は infra/README.md)。

手でバックアップを取るときは、その環境の compose ディレクトリで実行する
(本番は `.env` の `COMPOSE_PROJECT_NAME` が効くのでコマンドは共通):

```sh
# バックアップ (カスタム形式)
# (-T は TTY 割り当てを止めるオプション。付けないとバイナリ出力が壊れることがある)
docker compose exec -T db pg_dump -U discalendar -d discalendar -Fc > discalendar_$(date +%Y%m%d%H%M%S).dump
```

復元は**空の DB に対して**行う。R2 のバックアップから staging に入れる場合:

```sh
# 1. R2 から落とす (一覧の見方は infra/README.md。落とすのは staging ホストの作業ディレクトリで)
sudo BACKUP_ENV_FILE=/etc/discalendar/production.env /opt/discalendar-backup/r2.sh \
  s3 cp "s3://discalendar-backups/production/discalendar-20260827T190000Z.dump" .

# 2. staging の compose ディレクトリで、db だけ起動して復元する
#    (api を先に起動すると起動時マイグレーションが空 DB に走り、復元と衝突する。
#     bot も起動していると本番と同じ予定で通知が飛ぶので、その間は bot プロファイルを外す)
docker compose down
docker compose up -d db
docker compose exec -T db psql -U discalendar -d postgres \
  -c 'DROP DATABASE discalendar' -c 'CREATE DATABASE discalendar OWNER discalendar'
docker compose exec -T db pg_restore -U discalendar -d discalendar --no-owner --no-privileges < <ダンプファイル>
docker compose up -d
```

- サポート用 Bot のデータベースを戻すときは、support-bot を止めて同じ手順で `discalendar_support` を作り直し、
  `discalendar_support-<時刻>.dump` を入れる (本体の DB とは独立しているので、片方だけ戻してよい)
- 既存の DB に上書きで復元すると、マイグレーション履歴 (`_sqlx_migrations`) と実体が食い違って api が起動しなくなる。
  上のように DB を作り直してから入れる
- 旧版 (postgres 13、オーナー `postgres`) からの移行では `--no-owner --no-privileges` が必須。旧 DB 側では
  `pg_dump -Fc --no-owner --no-privileges -U postgres <DB名>` で取得する (切替手順の全体は #12)

## DB を書き換えるマイグレーションを適用するとき

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

## マイグレーションが入った版から戻す

イメージだけ戻しても DB は戻らない。**戻す先より後に入ったマイグレーションがあるときは、DB を先に戻す**:

- 古い api / bot は変更後のスキーマを読めない (例: `notifications` を `TEXT[]` としてデコードするので、
  JSONB のままだと予定のクエリが失敗し続ける)
- `sqlx::migrate!` は既定 (`ignore_missing = false`) なので、**自分の `migrations/` に無いバージョンが
  `_sqlx_migrations` にあるだけで api の起動が失敗する**

support-bot (サポート用 DB) のマイグレーションは `support-bot/rollback/` に同じ形で置いてある (流す先は `-d discalendar_support`)。

戻すための SQL は `api/rollback/<マイグレーションと同じ version>_*.sql` に置いてある (`migrations/` ではないので自動実行はされない)。
ファイル冒頭に手順があり、要点は「api / bot を止める → ダンプを取る → SQL を流す → 前の版のタグでデプロイ」。
DB を変えるマイグレーションを追加する PR では、対になる戻し方をここに用意する。
