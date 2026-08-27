# DisCalendar — AI エージェント向けガイド

Discord 用の共有カレンダー [DisCalendar](https://discalendar.app) を旧版 (Nuxt 2 + Rust) から作り直しているリポジトリ。
構成と開発手順は [README.md](README.md)、技術選定と進め方は [docs/tech-stack-selection.md](docs/tech-stack-selection.md) を参照。

- `web/`: Next.js 16 (App Router) + React 19 + FullCalendar v7 + Better Auth + TanStack Query + shadcn/ui (Base UI ベース)。
  Next.js の API は学習データと違うことがあるので、`web/AGENTS.md` の注意に従い `web/node_modules/next/dist/docs/` を確認してから書く
- `api/`: Rust (actix-web 4 + sqlx 0.9 + PostgreSQL)。詳細は [api/README.md](api/README.md)
- `bot/`: Rust (poise 0.6 + serenity 0.12 + sqlx 0.9)。ギルドの参加・退出・更新を `guilds` テーブルに反映し、
  スラッシュコマンド (help / create / list / init / invite、オーナー用 register) を提供する。定期タスク (#4) は移行中。
  予定の保存形式 (JST naive / 通知の JSONB / 終日予定の表現) は api と揃える。詳細は [bot/README.md](bot/README.md)
- `infra/`: SaaS 側の設定 (Terraform。`terraform/` = Cloudflare (provider 5 系)、`terraform/grafana/` = Grafana Cloud の
  ログアラート (provider 4 系)。どちらもバージョン固定・state 別) と、本番ホストで動かす DB バックアップ
  (pg_dump → R2、systemd timer)・ログ集約の Alloy 設定 (`alloy/`)。
  秘密情報は置かない (`*.tfvars` / `backend.hcl` は git 管理外)。詳細は [infra/README.md](infra/README.md)
- 旧実装は `tmp/DisCalendarV2/` (git 管理外) にある。移行時の挙動の根拠はそこを見る

## コマンド

- web (`web/` で実行): `pnpm lint` (Biome) / `pnpm exec tsc --noEmit` / `pnpm test` (Vitest) / `pnpm build`。
  E2E は `pnpm e2e` (Playwright。api + Postgres + Next を自動で立てる。Discord はモック。手順は README の「テスト」)
- api / bot (ルートで実行): `cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` (Postgres が必要)。
  `query!` を追加・変更したら該当クレートのディレクトリで `cargo sqlx prepare -- --all-targets` を実行し、
  `api/.sqlx/` / `bot/.sqlx/` を更新する (CI は `SQLX_OFFLINE=true`)
- infra: `terraform fmt -check -recursive` (`infra/terraform/` で) と、ルートモジュールごとの
  `terraform init -backend=false` → `terraform validate` (`infra/terraform/` と `infra/terraform/grafana/` の両方)。
  `plan` / `apply` は認証情報が要るので手元だけ (CI は書式と構文のみ)。`infra/backup/*.sh` は `shellcheck`、
  `infra/alloy/config.alloy` は `docker run --rm -v "$PWD/infra/alloy:/etc/alloy:ro" grafana/alloy:<版> validate /etc/alloy/config.alloy`
- コメント・ドキュメント・PR の文章は日本語
- **利用者に見える機能追加・変更・不具合修正をしたら、更新履歴 (`web/src/content/changelog.mdx`) に同じ PR で追記する**。
  利用者に伝わる言葉で書く (技術的な変更はそれ自体を書かず、利用者から見える効果に言い換える)。書き方のルールはファイル冒頭のコメント
- LP と使い方に貼っているスクリーンショット (`web/src/assets/`) は `pnpm shot` で撮り直す。
  手順と確認観点は `.agents/skills/update-screenshots/` (正本は `.claude/skills/`)
- **バージョン (v3.x.y) は web / api / bot で共通**。`web/package.json` / `api/Cargo.toml` / `bot/Cargo.toml` / `Cargo.lock` の 4 か所を揃えて
  リリースのたびに上げる (機能 PR では上げない)。CI の `version` ジョブが `.github/scripts/check-versions.sh` でずれを弾く。
  上げ幅の決め方・タグ (`v3.x.y`) の打ち方・本番デプロイは `.agents/skills/release/`
- Issue 着手から PR・レビュー対応までの手順は `.agents/skills/issue-driven-dev/`、worktree や `target/` の後片付けは `.agents/skills/cleanup-workspace/` にまとめてある。
  `.agents/skills/` は Codex のエントリポイントで、Claude Code と共有する詳細手順の正本は `.claude/skills/`

## AI エージェントのローカル / クラウド環境

- Codex デスクトップアプリの managed worktree は `.worktreeinclude` に従って ignored な `.env` をコピーし、Local environment の
  `bash .agents/scripts/setup-worktree-environment.sh` で依存を準備する。Claude Code から手動で worktree を作る場合は `issue-driven-dev` スキルのスクリプトを使う
- Codex cloud の Setup script は `bash .agents/scripts/setup-cloud-environment.sh --install-tools`、Maintenance script は
  `bash .agents/scripts/setup-cloud-environment.sh`。同じスクリプトを Claude Code の SessionStart hook からも呼ぶ

### クラウドセッションでの注意

claude.ai/code や `claude --cloud` のセッション (環境変数 `CLAUDE_CODE_REMOTE=true`) は Anthropic 管理の VM にこのリポジトリを clone して動く。
Codex cloud もタスクごとの隔離環境に clone して動く。セッション開始時に共通スクリプトが Postgres の起動と `api/migrations` の適用・
`web/` の `pnpm install`・ダミー値の `.env` 生成を行い、結果を `[agent-setup] ...` で報告する (Postgres が「未準備」なら、その指示に従って直してから
`cargo test` / `cargo sqlx prepare` に進む)。環境 (Environment) 側の設定手順は [README.md](README.md) の「AI エージェントの環境を整える」。

- **旧実装 `tmp/DisCalendarV2/` は無い** (git 管理外)。挙動の根拠が要るときは `docs/`・Issue・PR の記述で代用し、確認できなければ PR 本文にその旨を書く
- **対話ブラウザと Discord ログインでの動作確認はできない**。検証は CI 相当のコマンド (`pnpm lint` / `tsc` / `pnpm build`、
  `cargo fmt` / `clippy` / `test`) まで。UI の見た目や Discord 連携の確認は PR の「動作確認」に未実施として残し、ユーザーに引き継ぐ
- clone 内に追加の worktree は作らない (セッション自体が隔離されている)。Claude Code cloud では `git push` はセッションのカレントブランチにしかできない
- 秘密情報 (Bot トークン・Client Secret・本番の `BETTER_AUTH_SECRET`) は環境に入っていない前提で進める。`.env` の値はダミー

## Code Review Rules

PR レビュー (Codex / Claude) では以下を優先し、指摘は日本語で書く。

- CI (Biome / tsc / next build / rustfmt / clippy / cargo test) が検出する問題は指摘しない
- **P0: DB スキーマの互換性**。`api/migrations/` の適用済みファイルは `_sqlx_migrations` のチェックサムと
  照合されるため変更禁止 (旧版から引き継いだ 2 ファイルを含む)。スキーマの変更は必ず新しいファイルで行う。
  旧 Bot / 旧 Web との共有による凍結 (カラム型・通知設定の保存形式) は #15 で解除済みだが、
  `events` などは api と bot が同じ形で読み書きするので、変えるときは両方の対応と `.sqlx/` の更新を同じ PR に含める。
  既存データの持ち方が変わる変更 (型変換など) は、**対になる戻し方を `api/rollback/<version>_*.sql` に用意する**
  (イメージを戻しても DB は戻らず、古い api は起動すらできない。README の「マイグレーションが入った版から戻す」)
- **P0: 認可の迂回**。api 側の権限チェック (restricted モード、`can_manage_server`、`guild_id` + `event_id` での絞り込み) や
  Better Auth セッション検証を弱める変更。web 側の表示制御だけで済ませてはいけない
- **P0: 秘密情報**。`.env*` の内容、Bot トークン、セッション cookie の値、`BETTER_AUTH_SECRET` をコード・ログ・テスト・コミットに含める変更
- P1: web と api の境界のずれ (`web/src/lib/api/types.ts` / `endpoints.ts` と `api/src/routes` / `api/src/models` の不一致、
  タイトル 32 文字・説明 1000 文字・通知 10 件などの上限値の不一致)
- P1: TanStack Query のキャッシュ更新漏れ (mutation 後に `setQueryData` / invalidate していない、楽観的更新のロールバック漏れ)
- P1: Discord の Snowflake ID を number で扱う箇所 (API 境界では必ず文字列)
- 文体・命名・軽微なリファクタの提案は最小限にする。shadcn の生成物 (`web/src/components/ui/`) には極力手を入れない方針
