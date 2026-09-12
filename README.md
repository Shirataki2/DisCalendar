# DisCalendar

[discalendar.app](https://discalendar.app)

Discord 用の共有カレンダー。ブラウザで予定を作成・編集し、Discord や端末へのプッシュ通知でお知らせする。
旧版 (Nuxt 2 + Rust) を作り直しているリポジトリ。

## 構成

| ディレクトリ | 役割・主な技術 |
|---|---|
| [web/](web/) | Web アプリ: Next.js 16 / React 19 / FullCalendar / Better Auth |
| [api/](api/README.md) | API: Rust / actix-web / sqlx |
| [bot/](bot/README.md) | Discord Bot: Rust / poise / serenity |
| [infra/](infra/README.md) | Terraform、DB バックアップ、ログ集約 |

api / bot はルートの Cargo workspace で管理し、Rust toolchain は `rust-toolchain.toml` で固定する。

## ローカルで起動する

前提: Node 22+ / pnpm / Rust (rustup) / PostgreSQL。web / api / bot は同じ DB を使い、
web と api の `BETTER_AUTH_SECRET`、api と bot の `DISCORD_BOT_TOKEN` を揃える。
環境変数の説明は各ディレクトリの `.env.example` を参照。

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
Discord ログインを通すには [Discord Developer Portal](https://discord.com/developers/applications) のアプリで
OAuth2 Redirects に `http://localhost:3000/api/auth/callback/discord` を登録し、
Client Secret を `.env.local` の `DISCORD_CLIENT_SECRET` に設定する。

サーバー選択画面の「Bot が参加しているサーバー」は API が `guilds` テーブル（Bot が書き込む）で判定する。
ローカルでは下の bot を起動すれば参加中のサーバーが登録される。Bot を動かさない場合は手で登録しておく:

```sh
psql -d discalendar_dev -c "INSERT INTO guilds (guild_id, name, avatar_url, locale) VALUES ('<guild_id>', '<name>', NULL, 'ja') ON CONFLICT DO NOTHING"
```

### bot

api を一度起動してマイグレーションを適用してから、別のターミナルで起動する。
Bot は参加中のサーバーを DB に反映する。招待・コマンド登録は [Bot の README](bot/README.md) を参照。

```sh
cd bot
cp .env.example .env
cargo run
```

Docker でまとめて動かす場合は [compose の手順](docs/operations.md#docker-compose-で動かす) を参照。

## 検証

```sh
# web/ で実行
pnpm lint
pnpm exec tsc --noEmit
pnpm test
pnpm build
pnpm e2e

# リポジトリルートで実行 (Rust のテストには PostgreSQL が必要)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

E2E の初期設定・専用 DB・スクリーンショット更新は [開発・テスト環境](docs/development.md#テスト)、
SQLx のクエリ情報更新は [API の README](api/README.md) を参照。

## 詳細ドキュメント

| 目的 | 参照先 |
|---|---|
| E2E、スクリーンショット、GitHub での開発、AI エージェントの環境設定 | [開発・テスト環境](docs/development.md) |
| API 連携、UI、MDX、PWA、ルート構成 | [Web の構成と実装](docs/web-architecture.md) |
| 管理者の設定、SQL コンソール、監査ログ | [管理コンソール](docs/admin.md) |
| compose、デプロイ、リリース、監視、DB のバックアップ・復元・ロールバック | [デプロイと運用](docs/operations.md) |
| VAPID 鍵、配信の仕組み、端末での確認 | [プッシュ通知](docs/push-notifications.md) |
| 予定変更の Webhook、配信・再試行、E2E | [Webhook](docs/webhooks.md) |
| 技術選定の背景 | [技術選定](docs/tech-stack-selection.md) |
| 実装・レビュー規約 | [AGENTS.md](AGENTS.md) |

README は概要と起動手順を扱い、詳しい設定・運用手順は上記ドキュメントに追記する。

### DB を書き換えるマイグレーションを適用するとき

[デプロイと運用の該当手順](docs/operations.md#db-を書き換えるマイグレーションを適用するとき)を参照。
適用済みマイグレーション内の旧参照を維持するため、この見出しを残している（SQL ファイルはコメント変更でもチェックサムが変わるため編集禁止）。
