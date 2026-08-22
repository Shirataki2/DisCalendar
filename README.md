# DisCalendar

> <https://discalendar.app>

Discord 用のカレンダーアプリ。予定の作成から通知まで、ブラウザから面倒なコマンド操作なしで扱える。

このリポジトリは稼働中の DisCalendar（旧コード: `tmp/DisCalendarV2`、git 管理外）をモダナイズするプロジェクト。最終的に front / api / bot をこのリポジトリ配下に集約する。

## 構成

| ディレクトリ | 内容 | 状態 |
|---|---|---|
| `web/` | Next.js 16 (App Router) + React 19 + FullCalendar v7 + Better Auth + TanStack Query | Discord ログイン、サーバー選択、カレンダー (API 接続済み: 予定の取得 / 作成 / 移動 / 削除) |
| `api/` | Rust API（actix-web 4 + sqlx 0.9、旧版から移行） | 移行済み（[README](api/README.md)） |
| `bot/` | Discord Bot（Rust、旧版から移行予定） | 未着手 |
| `docs/` | 技術選定・設計ドキュメント | [技術選定](docs/tech-stack-selection.md) |

Rust 側（api / bot）はルートの `Cargo.toml` を workspace とし、`rust-toolchain.toml` で toolchain を固定している。

### web と api のつなぎ方

- ブラウザからの API 呼び出しは同一オリジンの `/local/api/*` に投げ、`web/next.config.ts` の rewrites が
  `API_URL`（既定 `http://127.0.0.1:8080`）へプロキシする。Better Auth のセッション cookie がそのまま転送され、
  API 側がそれを検証する（旧実装の `@nuxtjs/proxy` と同じ構成）。
- Server Component からは `web/src/lib/api/server.ts` が cookie を付けて API を直接呼ぶ。
- クライアント側のデータ取得・更新は TanStack Query（`web/src/lib/query/`）。ドラッグ移動などは楽観的更新し、失敗時は元に戻す。
- API のエンドポイント定義は `web/src/lib/api/endpoints.ts`（Rust 側の `api/src/routes` と対応）。

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

Discord ログインを通すには [Discord Developer Portal](https://discord.com/developers/applications) のアプリで
OAuth2 Redirects に `http://localhost:3000/api/auth/callback/discord` を登録し、
Client Secret を `.env.local` の `DISCORD_CLIENT_SECRET` に設定する。

サーバー選択画面の「Bot が参加しているサーバー」は API が `guilds` テーブル（本番では Bot が書き込む）で判定する。
ローカルではこのテーブルが空なので、Bot が実際に参加しているサーバーを手で登録しておく:

```sh
psql -d discalendar_dev -c "INSERT INTO guilds (guild_id, name, avatar_url, locale) VALUES ('<guild_id>', '<name>', NULL, 'ja') ON CONFLICT DO NOTHING"
```

## ルート構成 (web)

| パス | 内容 |
|---|---|
| `/` | ランディング |
| `/login` | Discord ログイン |
| `/dashboard` | サーバー選択（Bot 参加済み / 招待可能なサーバー） |
| `/dashboard/[id]` | ギルドごとのカレンダー |
| `/api/auth/*` | Better Auth（OAuth コールバック含む） |
| `/local/api/*` | Rust API へのプロキシ（rewrites） |
