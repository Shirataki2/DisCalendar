# Issue / PR の記入例

GitHub 上のフォーム (`.github/ISSUE_TEMPLATE/*.yml`) は `gh issue create` からは使えないので、
同じ見出しを Markdown で書いて `--body-file` に渡す。`blank_issues_enabled: false` だが CLI からの作成は通る。

## Issue: 開発タスク (`[task]`、ラベル `task` + `area:*`)

```markdown
## 作業内容

<何をするか。背景や参照先 (docs/tech-stack-selection.md の節、旧実装 tmp/DisCalendarV2 のファイルなど)>

## 完了条件

- [ ] <どうなったら閉じてよいか (チェックリスト)>
- [ ] README / docs を更新

## 対象

<web / api / bot / infra / docs>

## メモ

<実装方針、懸念、関連 Issue / PR (任意)>
```

```bash
gh issue create --title "[task] <一文>" --label task --label area:web --milestone "v3 リリース" --body-file body.md
```

## Issue: 不具合報告 (`[bug]`、ラベル `bug` + `area:*`)

```markdown
## 概要

## 再現手順

1. /dashboard/<サーバー> を開く
2. ...

## 期待する動作

## 実際の動作

<エラーメッセージ・ログ (トークンなどは伏せる)>

## 発生箇所

<web / api / bot / 不明>

## 環境

<ブラウザ / OS (web の場合)>
```

## Issue: 機能要望 (`[feature]`、ラベル `enhancement` + `area:*`)

```markdown
## 背景・課題

## 提案

<旧版 (discalendar.app) との違いがあれば書く>

## 代替案・検討したこと

## 対象
```

振る舞いを変えない整理は `[refactor]` + ラベル `refactor` (本文は task と同じ形でよい)。

## PR 本文 (`.github/PULL_REQUEST_TEMPLATE.md` に沿った記入例)

```markdown
## 概要

#4 の実装。旧 Bot (`tmp/DisCalendarV2/bot/src/tasks/`) の定期タスク 3 つを新 `bot/` に移す。

## 関連 Issue

Closes #4

## 変更内容

- notify: 60 秒ごとに通知対象の予定を引き、`event_settings` の通知先チャンネルへ embed を送る (終日予定の丸めは旧実装を踏襲)
- presence: 10 秒ごとに `/help` → サーバー数 → URL を順送り
- icon_updater: JST 0:00 に日付アイコンへ更新 (`bot/assets/` に旧画像をコピー)
- Dockerfile に `COPY bot/assets` を追加 (実行時にアイコンを読めなかったため)

## 動作確認

- [x] `cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace`
- [x] notify の判定ロジック・終日予定の丸めを単体テストで確認
- [ ] 実機確認 (テスト用サーバーで `/init` → `/create` → 通知到達) はユーザーが実施

## チェックリスト

- [x] api / bot: fmt / clippy / test が通る
- [x] sqlx のクエリを追加したので `bot/` で `cargo sqlx prepare -- --all-targets` を実行して `.sqlx/` を更新した
- [x] DB スキーマは変えていない
- [ ] README / docs を更新した (bot/README.md に定期タスクの節を追加)
- [x] `.env*` や秘密情報を含めていない
- [ ] Claude / Codex のレビュー指摘を確認し、対応したか理由をコメントした

<!-- エージェント固有の生成フッタは不要 -->
```

```bash
gh pr create --title "bot: 定期タスク (予定の通知 / presence / 日付アイコン更新) を移行する" \
  --milestone "v3 リリース" --label area:bot --body-file pr-body.md
```

ポイント:

- タイトルは squash コミットのメッセージになる。「〜を追加する」「〜を移行する」のような一文にし、末尾に `(#N)` は付けない (GitHub が PR 番号を付ける)
- 「動作確認」には実際にやったことだけを書き、未実施は未チェックで残す。レビュアー (Claude / Codex / ユーザー) がそこを見て判断する
- Issue の完了条件で PR に含めないものは、どこへ移したかを書く (例: 「旧 Bot と同時起動しない手順は #12 にコメント済み」)

## レビューコメントへの返信

```bash
# 行コメント一覧 (id を控える)
gh api repos/{owner}/{repo}/pulls/<N>/comments --jq '.[] | "\(.id) \(.user.login) \(.path):\(.line)\n\(.body)\n"'
# 返信 (対応した / 対応しない理由)
gh api repos/{owner}/{repo}/pulls/<N>/comments/<comment-id>/replies -f body='対応しました: <コミット SHA> で ...'
# PR 全体へのコメント
gh pr comment <N> --body '...'
```

## 環境変数を足すときの確認先

web の `NEXT_PUBLIC_*` は `next build` 時にインライン化されるため、実行時の環境変数では効かない (Docker イメージに焼き込む)。
`API_URL` (rewrites の転送先) が同じ扱いなので、それを手本にする。

| 場所 | 何を書くか |
|---|---|
| `web/.env.example` / `api/.env.example` / `bot/.env.example` | 変数名・意味・未設定時の挙動をコメントで |
| ルート `.env.example` + `compose.yaml` | compose で渡す値 (`environment:` か `build.args:`) |
| `web/Dockerfile` | ビルド時に必要なら `ARG` を build ステージに追加 |
| `.github/workflows/deploy-staging.yml` | staging のビルド引数 / 環境変数 (Repository variables / secrets) |
| `.github/workflows/ci.yml` | CI のビルドに必要ならダミー値 (`BETTER_AUTH_SECRET` と同様) |
| ルート `README.md` | 設定手順 (ドキュメントの正はここ) |
| `api/src/config.rs` 相当 / `bot` の設定読み込み | Rust 側は起動時の読み込みと必須チェック |
