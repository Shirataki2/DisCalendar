# DisCalendar

> <https://discalendar.app>

Discord 用のカレンダーアプリ。予定の作成から通知まで、ブラウザから面倒なコマンド操作なしで扱える。

このリポジトリは稼働中の DisCalendar（旧コード: `tmp/DisCalendarV2`、git 管理外）をモダナイズするプロジェクト。最終的に front / api / bot をこのリポジトリ配下に集約する。

## 構成

| ディレクトリ | 内容 | 状態 |
|---|---|---|
| `web/` | Next.js 16 (App Router) + React 19 + FullCalendar v7 + Better Auth | カレンダー PoC + Discord ログイン |
| `api/` | Rust API（actix-web、旧版から移行予定） | 未着手 |
| `bot/` | Discord Bot（Rust、旧版から移行予定） | 未着手 |
| `docs/` | 技術選定・設計ドキュメント | [技術選定](docs/tech-stack-selection.md) |

## 開発

前提: Node 22+ / pnpm / ローカルの PostgreSQL

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

http://localhost:3000 でランディング、`/login` から Discord ログイン、
`/dashboard` でサーバー選択 → 各サーバーのカレンダーが開く。

Discord ログインを通すには [Discord Developer Portal](https://discord.com/developers/applications) のアプリで
OAuth2 Redirects に `http://localhost:3000/api/auth/callback/discord` を登録し、
Client Secret を `.env.local` の `DISCORD_CLIENT_SECRET` に設定する。

## ルート構成 (web)

| パス | 内容 |
|---|---|
| `/` | ランディング |
| `/login` | Discord ログイン |
| `/dashboard` | サーバー選択（所属ギルド一覧） |
| `/dashboard/[id]` | ギルドごとのカレンダー |
| `/api/auth/*` | Better Auth（OAuth コールバック含む） |
