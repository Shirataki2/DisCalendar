# DisCalendar

> <https://discalendar.app>

Discord 用のカレンダーアプリ。予定の作成から通知まで、ブラウザから面倒なコマンド操作なしで扱える。

このリポジトリは稼働中の DisCalendar（旧コード: `tmp/DisCalendarV2`、git 管理外）をモダナイズするプロジェクト。最終的に front / api / bot をこのリポジトリ配下に集約する。

## 構成

| ディレクトリ | 内容 | 状態 |
|---|---|---|
| `web/` | Next.js 16 (App Router) + React 19 + FullCalendar v7 | スキャフォールド + カレンダー PoC |
| `api/` | Rust API（actix-web、旧版から移行予定） | 未着手 |
| `bot/` | Discord Bot（Rust、旧版から移行予定） | 未着手 |
| `docs/` | 技術選定・設計ドキュメント | [技術選定](docs/tech-stack-selection.md) |

## 開発

```sh
cd web
pnpm install
pnpm dev
```

http://localhost:3000 でランディング、`/dashboard` でカレンダー PoC が開く。
