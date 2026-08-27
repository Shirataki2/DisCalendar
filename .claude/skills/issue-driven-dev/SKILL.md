---
name: issue-driven-dev
description: DisCalendar の GitHub Issue を起点にした開発フロー (Issue の確認・作成 → worktree とブランチの用意 → 実装・検証 → PR 作成 → CI と Claude / Codex レビューへの対応 → マージ後の後始末) を、このリポジトリのルールどおりに進めるためのスキル。「Issue #N をやって」「#N の実装を始めて」「Issue を切って」「これを Issue にして進めて」「PR を作って」「レビュー指摘に対応して」など Issue 番号・Issue / PR 作成・レビュー対応に触れる依頼や、機能追加・移行・修正などの作業単位を始めるときに使う。main 直接 push 禁止、ラベル・マイルストーン、PR テンプレート、worktree の作り方といったリポジトリ固有の決まりをここに集約している。
---

# Issue 駆動開発 (DisCalendar)

1 つの Issue を 1 本のブランチ・1 つの PR で片付け、`Closes #N` で Issue を閉じる。
ルールの多くは GitHub 側のルールセットで強制されている (main への直接 push は拒否される) ので、
最初から手順に沿った方が早い。

## リポジトリのルール (先に押さえる)

- `main` は直接 push 禁止。必ず ブランチ → PR → 必須チェック `ci` 成功 → **squash マージ** (マージ後リモートブランチは自動削除)
- Issue はテンプレートに沿う: タイトル接頭辞 `[task]` / `[bug]` / `[feature]`、ラベルは種類 (`task` / `bug` / `enhancement` / `refactor`) + 対象 (`area:web` / `area:api` / `area:bot` / `area:infra`)、必要なら `priority:high`。
  マイルストーンは **「v3 リリース」1 本だけ** (ステップ単位には分けない)
- PR のタイトルは squash 後のコミットメッセージになるので「変更内容が分かる一文」。本文は `.github/PULL_REQUEST_TEMPLATE.md` の構成で、`Closes #N` を入れる
- 実装規約は [AGENTS.md](../../../AGENTS.md) (+ `web/AGENTS.md`, `api/README.md`, `bot/README.md`)。
  Code Review Rules の P0 (DB スキーマ互換・認可の迂回・秘密情報) はレビューで必ず弾かれるので最初から守る
- 旧実装は `tmp/DisCalendarV2/` (git 管理外。**メインの checkout にしか無い**ので worktree からは絶対パスで参照する)
- squash マージのため `git branch --merged` や `merge-base` ではマージ済みか分からない。判定は PR の状態 (`gh pr list --head <branch> --state all`) で行う
- コミットメッセージ・PR・Issue・コードコメントは日本語

## 全体の流れ

| 段階 | やること | 主な道具 |
|---|---|---|
| 0 | Issue を読む / 無ければ作る | `gh issue view` / `gh issue create` |
| 1 | worktree とブランチを用意する | Codex の Worktree モード、または `scripts/setup-worktree.sh` |
| 2 | 実装と検証 | AGENTS.md のコマンド、ブラウザ / Playwright |
| 3 | コミット → push → PR | `gh pr create` |
| 4 | CI とレビュー対応 | `gh pr checks` / `gh api .../comments` |
| 5 | マージ後の後始末 | `cleanup-workspace` スキル |

### 0. Issue を決める

- Issue を読む (`--comments` は非 TTY でコメント 0 件だと何も出ないので JSON で取る):

  ```bash
  gh issue view <N> --json title,body,labels,milestone,comments \
    --jq '.title, (.labels|map(.name)|join(",")), .milestone.title, .body, (.comments[] | "--- \(.author.login):\n\(.body)")'
  ```

  「作業内容 / 完了条件 / 対象 / メモ」とコメントを押さえ、完了条件が複数あれば利用できる計画・Todo 機能に反映する。分からない点は、まず旧実装 (`tmp/DisCalendarV2/`)・`docs/`・関連 Issue / PR を見てから質問する
- Issue が無い作業を頼まれたら、先に Issue を作ってから着手する (進捗が Issue 一覧に残るようにする意図)。
  本文の形は [references/templates.md](references/templates.md) を見る。例:

  ```bash
  gh issue create --title "[task] 予定一覧の CSV エクスポートを追加する" \
    --label task --label area:web --label area:api --milestone "v3 リリース" --body-file body.md
  ```

  Issue 作成は外向きの操作なので、タイトル・本文をユーザーに見せてから実行する (ユーザーが「Issue にして」と言っている場合はそのまま作ってよい)
- 作業中に Issue の範囲外の問題に気づいたら、その PR に混ぜずに別 Issue を切る。1 PR = 1 Issue を保つとレビューも squash 後の履歴も読みやすい

### 1. worktree とブランチ

既定は worktree で作業する (メインの checkout を `main` のまま汚さず、複数の Issue を並行でき、dev サーバーも別々に立てられる)。

Codex デスクトップアプリでは、新しいタスクを作るときに **Worktree** を選ぶ。アプリが `$CODEX_HOME/worktrees` に detached HEAD の
worktree を作り、`.worktreeinclude` に列挙したローカル設定をコピーする。セットアップにはリポジトリの Local environment
(`bash .agents/scripts/setup-worktree-environment.sh`) を選ぶ。作業を残すときはアプリの **Create branch here** で `codex/issue-<N>-<slug>` を作るか、
**Handoff** で Local に移す。

**クラウドセッション (Codex cloud / claude.ai/code / `claude --cloud`) では、clone 済みのカレント checkout 内に追加の worktree を作らない。**
セッション自体が隔離されているので、この節は飛ばしてカレントブランチのまま 2 以降へ進む。旧実装 `tmp/DisCalendarV2/` と
実トークンは無く、Discord ログインや実機確認はできない (検証は CI 相当まで。AGENTS.md「クラウドセッションでの注意」)。

```bash
# <N>: Issue 番号、<slug>: 英小文字とハイフン 2〜4 語 (例: bot-tasks, staging-deploy, ga4)
.claude/skills/issue-driven-dev/scripts/setup-worktree.sh <N> <slug>            # Rust だけなら
.claude/skills/issue-driven-dev/scripts/setup-worktree.sh <N> <slug> --install  # web を触るなら pnpm install も
```

スクリプトは `.claude/worktrees/issue-<N>-<slug>` に `origin/main` 起点のブランチを作る。既定の prefix は Codex なら `codex/`、
Claude Code なら `claude/` で、`--prefix` でも指定できる。
git 管理外の `.env` 系 (`api/.env`, `web/.env.local` など) をメインの checkout からコピーする。
Claude Code で `EnterWorktree` が使える場合は、終わったら `path` に表示されたパスを渡してセッションを移す。Codex やサブエージェントなど
セッションを移せない環境では、表示された絶対パスを workdir / cwd にして作業する。

- サブエージェントなど cwd が固定された環境では、セッションを移さず worktree の絶対パスで作業する (`git -C <path>`, `pnpm -C <path>/web`, `cargo ... --manifest-path <path>/Cargo.toml` か `cd <path> && ...`)
- `--install` を付けずに作った後で web を触ることになったら `pnpm -C <path>/web install --frozen-lockfile`

- 既にブランチがある場合 (GitHub Actions の `@claude` が `claude/issue-N-...` を作った、前回の続きなど): `--branch <ブランチ名>` を付けると、そのブランチ (ローカル or `origin/`) を checkout する
- Claude Code の worktree セッションで別パスへの `git -C` が拒否される場合は、先に `ExitWorktree` (`keep`) で戻ってからスクリプトを実行する。Codex では Handoff か別の Local タスクを使う
- 小さな変更 (ドキュメント 1 ファイルなど) でユーザーが望めばメインの checkout で `git switch -c` でもよいが、
  終わったら必ず `main` に戻す

worktree 内で気をつけること:

- web の `node_modules` と Rust の `target/` は worktree ごとに別 (`target/` は数百 MB〜数 GB)。終わったら `cleanup-workspace` で消す
- dev サーバーのポートはメインの checkout と衝突する (web 3000 / api 8080)。`.claude/launch.json` で起動する前に、他で動いていないか確認する
- `.claude/launch.json` には bot もある (`cargo run`)。bot は HTTP を待ち受けないのでプレビューのタブは開けないが、`preview_logs` でログは追える。
  ポート 8081 は衝突検知のための飾り。**同じトークンの Bot が他 (本番 / staging / メインの checkout) で動いていると通知が二重に飛ぶ**ので、起動前に確認する
- Rust のテストと `cargo sqlx prepare` はローカル Postgres が要る。`DATABASE_URL` は各クレートの `.env` (`api/.env` / `bot/.env`、`bot/README.md` のとおり api と同じ値) から読まれる。
  `bot/.env` を作っていない環境では `api/.env` の値を環境変数で渡す (例: `DATABASE_URL=$(grep '^DATABASE_URL=' api/.env | cut -d= -f2-) cargo test -p discalendar-bot`)。接続先をスキルに決め打ちしない

### 2. 実装と検証

- Issue の完了条件を満たす最小の変更にする。設計判断で迷ったら旧実装の挙動を根拠にし、PR 本文に理由を書く
- 変更した領域の検証を手元で通す (CI と同じ):
  - web (`web/` で): `pnpm lint` / `pnpm exec tsc --noEmit` / `pnpm test` / `pnpm build`。カレンダー周りを触ったら `pnpm e2e` (Playwright、README「テスト」)
  - api / bot (ルートで): `cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace`
  - `query!` を触ったら該当クレートで `cargo sqlx prepare -- --all-targets` を実行して `.sqlx/` をコミットに含める (CI は `SQLX_OFFLINE=true`)
- UI の変更は利用できるブラウザまたは Playwright で実際に動かして確認し、PR の「動作確認」に手順を書く
- 利用者に見える機能追加・変更・不具合修正なら、更新履歴 (`web/src/content/changelog.mdx`) に同じ PR で追記する (書き方はファイル冒頭のコメント。利用者に見えない内部変更は書かない)
- web と api の境界 (`web/src/lib/api/types.ts` / `endpoints.ts` と `api/src/routes` / `api/src/models`、上限値) を変えたら両側を揃える
- 環境変数を追加・変更するときは [references/templates.md の「環境変数を足すときの確認先」](references/templates.md#環境変数を足すときの確認先) を見る (web の `NEXT_PUBLIC_*` はビルド時に焼き込まれるので Dockerfile / compose / デプロイワークフローにも手が要る)
- ドキュメントの正はルート `README.md` (`web/README.md` は create-next-app の雛形のまま)。設定手順などはそちらに書く

### 3. コミット → push → PR

```bash
git add -A && git commit -m "<日本語の要約>"
git push -u origin "$(git rev-parse --abbrev-ref HEAD)"
gh pr create --title "<squash 後のコミットメッセージになる一文>" \
  --milestone "v3 リリース" --label area:<対象> --body-file pr-body.md
```

- PR 本文は `.github/PULL_REQUEST_TEMPLATE.md` の見出し (概要 / 関連 Issue / 変更内容 / 動作確認 / チェックリスト) を守る。
  記入例は [references/templates.md](references/templates.md)。チェックリストは **実際に確認した項目だけ** チェックし、未実施 (実機確認待ちなど) は未チェックのまま明記する
- Issue の完了条件のうち PR に含めないもの (コード変更でない手順、別 Issue に移すもの) があれば、PR 本文と Issue のコメントに行き先を書く
- `gh pr merge --auto --squash` (CI 通過で自動マージ) は **ユーザーが明示的に望んだときだけ** 有効にする。
  実機確認 (Discord での動作確認など) を待つ PR をうっかりマージしないため

### 4. CI とレビュー対応

- `gh pr checks <N> --watch` で `ci` を待つ。落ちたら `gh run view <run-id> --log-failed` で原因を見る
- Claude (`claude-code-review.yml`) と Codex がレビューコメントを付ける。取り方:
  - PR 全体のコメント: `gh pr view <N> --comments`
  - 行コメント: `gh api repos/{owner}/{repo}/pulls/<N>/comments --jq '.[] | {id, path, line, user: .user.login, body}'`
  - レビュー本体: `gh api repos/{owner}/{repo}/pulls/<N>/reviews --jq '.[] | {id, user: .user.login, state, body}'`
- 指摘ごとに「直す」か「直さない理由を返信する」かを決める (PR チェックリストの最後の項目)。
  返信: `gh api repos/{owner}/{repo}/pulls/<N>/comments/<comment-id>/replies -f body='...'`。
  判断の根拠は AGENTS.md の Code Review Rules (P0 / P1 の優先度、文体の指摘は最小限)。誤検知なら根拠を示して返信する
- push のたびに CI とレビューが再実行され、進行中のものはキャンセルされる。指摘への対応はまとめて 1 回 push する

### 5. マージ後

- `Closes #N` で Issue は自動で閉じる。残作業があれば Issue を開け直さず、別 Issue かコメントに書く
- メインの checkout を最新にする: (Claude Code は `ExitWorktree`、Codex は Handoff か Local タスクで戻ってから) `git switch main && git pull --ff-only`
- worktree・ローカルブランチ・`target/` の掃除は **`cleanup-workspace` スキル** で行う (PR の状態を見て安全に消す)
- 移行で分かった非自明な挙動 (旧実装との差、ライブラリの罠) は、後続の作業でも参照できるドキュメントや Issue に残す
