# 開発・テスト環境

[README に戻る](../README.md)

パスとコマンドは、特記がなければリポジトリルートを基準に記載する。

## テスト

web のユニットテスト (Vitest) と E2E (Playwright) がある。api / bot のテストは `cargo test --workspace` (Postgres が必要、[api/README.md](../api/README.md))。

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

設定は [web/playwright.config.ts](../web/playwright.config.ts) と [web/vitest.config.mts](../web/vitest.config.mts)。

### スクリーンショットの撮り直し (`pnpm shot`)

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

## 開発の進め方 (GitHub)

- 作業は Issue に登録する (`.github/ISSUE_TEMPLATE/` のフォーム: 不具合報告 / 機能要望 / 開発タスク)。
  マイルストーン (テーマ単位。現在の一覧は `gh api repos/{owner}/{repo}/milestones` で確認) と `area:*` ラベルを付けて進捗を追う
- `main` への直接 push は禁止 (ルールセット)。ブランチを切って PR を作り、本文の `Closes #N` で Issue と紐付ける
  (`gh issue develop N --checkout` でブランチを作れる)。マージは squash のみで、マージ後のブランチは自動削除される
- PR では CI (`.github/workflows/ci.yml`: web は Biome / tsc / Vitest / next build、rust (api / bot) は rustfmt / clippy / test、
  e2e は Playwright (web か rust に変更があるとき)) が通ることが必須
- AI レビュー: Claude (`.github/workflows/claude-code-review.yml`、secret `CLAUDE_CODE_OAUTH_TOKEN` が必要) と
  Codex (Codex クラウドの GitHub 連携で自動レビュー) が PR を確認する。コメントで `@claude` / `@codex review` と呼ぶと追加で依頼できる。
  レビューの観点は [AGENTS.md](../AGENTS.md) の「Code Review Rules」
- 依存の更新は Dependabot (`.github/dependabot.yml`) が毎週まとめて PR を出す

## AI エージェントの環境を整える

### Claude Code のスキルを Codex でも使う

スキルの詳細手順とスクリプトの正本は `.claude/skills/` に置き、Codex がリポジトリスキルとして検出する
`.agents/skills/` から正本を読む。片方だけを直して手順が分岐しない構成にしている。

- Codex CLI / IDE では `/skills` または `$issue-driven-dev` のように指定する。依頼が description に合えば自動選択もされる
- Claude Code では従来どおり `.claude/skills/` が使われる
- 対応するスキルは `issue-driven-dev` / `cleanup-workspace` / `release` / `update-screenshots`

Codex のスキル探索場所と形式は [Build skills](https://developers.openai.com/codex/skills) を参照。

### Codex のローカル worktree

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

### Codex / Claude Code のクラウド環境

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
