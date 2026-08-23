# DisCalendar — AI エージェント向けガイド

Discord 用の共有カレンダー [DisCalendar](https://discalendar.app) を旧版 (Nuxt 2 + Rust) から作り直しているリポジトリ。
構成と開発手順は [README.md](README.md)、技術選定と進め方は [docs/tech-stack-selection.md](docs/tech-stack-selection.md) を参照。

- `web/`: Next.js 16 (App Router) + React 19 + FullCalendar v7 + Better Auth + TanStack Query + shadcn/ui (Base UI ベース)。
  Next.js の API は学習データと違うことがあるので、`web/AGENTS.md` の注意に従い `web/node_modules/next/dist/docs/` を確認してから書く
- `api/`: Rust (actix-web 4 + sqlx 0.9 + PostgreSQL)。詳細は [api/README.md](api/README.md)
- `bot/`: Rust (poise 0.6 + serenity 0.12 + sqlx 0.9)。ギルドの参加・退出・更新を `guilds` テーブルに反映し、
  スラッシュコマンド (help / create / list / init / invite、オーナー用 register) を提供する。定期タスク (#4) は移行中。
  予定の保存形式 (JST naive / 旧形式の通知 JSON / 終日予定の表現) は api と揃える。詳細は [bot/README.md](bot/README.md)
- 旧実装は `tmp/DisCalendarV2/` (git 管理外) にある。移行時の挙動の根拠はそこを見る

## コマンド

- web (`web/` で実行): `pnpm lint` (Biome) / `pnpm exec tsc --noEmit` / `pnpm build`
- api / bot (ルートで実行): `cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` (Postgres が必要)。
  `query!` を追加・変更したら該当クレートのディレクトリで `cargo sqlx prepare -- --all-targets` を実行し、
  `api/.sqlx/` / `bot/.sqlx/` を更新する (CI は `SQLX_OFFLINE=true`)
- コメント・ドキュメント・PR の文章は日本語
- Issue 着手から PR・レビュー対応までの手順は `.claude/skills/issue-driven-dev/`、worktree や `target/` の後片付けは `.claude/skills/cleanup-workspace/` (Claude Code のプロジェクトスキル) にまとめてある

## Claude Code のクラウドセッションでの注意

claude.ai/code や `claude --cloud` のセッション (環境変数 `CLAUDE_CODE_REMOTE=true`) は Anthropic 管理の VM にこのリポジトリを clone して動く。
セッション開始時に `.claude/settings.json` の SessionStart hook (`.claude/hooks/cloud-session-start.sh`) が Postgres の起動と `api/migrations` の適用・
`web/` の `pnpm install`・ダミー値の `.env` 生成を行い、結果を `[cloud-session-start] ...` で報告する (Postgres が「未準備」なら、その指示に従って直してから
`cargo test` / `cargo sqlx prepare` に進む)。環境 (Environment) 側の設定手順は [README.md](README.md) の「Claude Code のクラウド環境で使う」。

- **旧実装 `tmp/DisCalendarV2/` は無い** (git 管理外)。挙動の根拠が要るときは `docs/`・Issue・PR の記述で代用し、確認できなければ PR 本文にその旨を書く
- **ブラウザ (Browser pane / Playwright) と Discord ログインでの動作確認はできない**。検証は CI 相当のコマンド (`pnpm lint` / `tsc` / `pnpm build`、
  `cargo fmt` / `clippy` / `test`) まで。UI の見た目や Discord 連携の確認は PR の「動作確認」に未実施として残し、ユーザーに引き継ぐ
- worktree は作らない (セッションごとに VM とブランチが分かれている)。`git push` はセッションのカレントブランチにしかできない
- 秘密情報 (Bot トークン・Client Secret・本番の `BETTER_AUTH_SECRET`) は環境に入っていない前提で進める。`.env` の値はダミー

## Code Review Rules

PR レビュー (Codex / Claude) では以下を優先し、指摘は日本語で書く。

- CI (Biome / tsc / next build / rustfmt / clippy / cargo test) が検出する問題は指摘しない
- **P0: DB スキーマの互換性**。`api/migrations/` の既存ファイルは旧版と `_sqlx_migrations` のチェックサムを共有しているため変更禁止。
  `events` などのテーブルは稼働中の旧 Bot も読み書きするので、カラムの型変更・削除・NOT NULL 追加や、
  通知設定 (`notifications`) の DB 上の保存形式の変更は Bot 移行が終わるまで不可
- **P0: 認可の迂回**。api 側の権限チェック (restricted モード、`can_manage_server`、`guild_id` + `event_id` での絞り込み) や
  Better Auth セッション検証を弱める変更。web 側の表示制御だけで済ませてはいけない
- **P0: 秘密情報**。`.env*` の内容、Bot トークン、セッション cookie の値、`BETTER_AUTH_SECRET` をコード・ログ・テスト・コミットに含める変更
- P1: web と api の境界のずれ (`web/src/lib/api/types.ts` / `endpoints.ts` と `api/src/routes` / `api/src/models` の不一致、
  タイトル 32 文字・説明 1000 文字・通知 10 件などの上限値の不一致)
- P1: TanStack Query のキャッシュ更新漏れ (mutation 後に `setQueryData` / invalidate していない、楽観的更新のロールバック漏れ)
- P1: Discord の Snowflake ID を number で扱う箇所 (API 境界では必ず文字列)
- 文体・命名・軽微なリファクタの提案は最小限にする。shadcn の生成物 (`web/src/components/ui/`) には極力手を入れない方針
