---
name: release
description: DisCalendar のバージョン (v3.x.y) 決めと本番リリースの手順。「リリースして」「本番に出して」「バージョンを上げて」「タグを打って」「v3.1.0 を出す」「リリースノートを作って」「本番にデプロイして」「前の版に戻して (ロールバック)」のような依頼のほか、機能追加をひととおり main にマージし終えて本番へ配るときに使う。semver の上げ幅の決め方 (DisCalendar では何が major / minor / patch か)、バージョンを持つ 4 か所の揃え方、v タグ push で走る release ワークフロー (GHCR の再タグ + GitHub Release)、本番デプロイとロールバックの手順をここに集約している。
---

# リリース (DisCalendar)

**web / api / bot は 1 つのバージョンを共有する** (モノレポで同時にデプロイするため)。
#12 の本番切替 (2026-08-27) を **v3.0.0** とし、それ以降は semver で上げていく。

リリースの実体は **git のタグ `v3.x.y`**。タグを push すると [`.github/workflows/release.yml`](../../../.github/workflows/release.yml) が
GHCR のイメージに `v3.x.y` を付け直し、更新履歴から作ったノートで GitHub Release を公開する。
**本番への反映だけは自動にしていない** (承認と「staging で確かめたものを配る」運用を残すため) ので、
最後に Actions の "Deploy production" を手で実行する。

## 上げ幅の決め方 (このリポジトリでの semver)

前回のタグ以降に main へ入った変更のうち、**利用者に見えるもの**で決める (内部のリファクタ・CI・依存更新だけなら上げない)。
判断材料は更新履歴 (`web/src/content/changelog.mdx`) の前回タグ以降のエントリ = リリースノートに載る内容そのもの。

| 上げ幅 | DisCalendar での目安 | 例 |
|---|---|---|
| **major** (`4.0.0`) | 利用者の使い方・データが変わる作り直し。旧版 → v3 のような全面刷新、Bot コマンド体系の入れ替え、DB スキーマの非互換な変更 | v3 そのもの (旧 Nuxt 版からの切替) |
| **minor** (`3.1.0`) | 機能追加・見た目や操作の変更。新しい画面・スラッシュコマンド・設定項目、API エンドポイントの追加 | テーマ切替 (#58)、日付ジャンプ (#59) |
| **patch** (`3.0.1`) | 不具合修正と、利用者に見える範囲を変えない改善 (表示崩れ・速度・文言の修正) | 404 ページの日本語化 (#61) |

- 迷ったら **上に寄せる** (機能追加か修正か曖昧なら minor)
- 複数の変更がまとまっている場合は、**一番大きいもの**に合わせる (修正 5 件 + 機能 1 件 → minor)
- 本番に出す前に staging で試したい版は プレリリース `3.1.0-rc.1` にできる (`latest` は付かず、Release は prerelease になる)

## バージョンを持つ場所 (4 か所)

`web/package.json` / `api/Cargo.toml` / `bot/Cargo.toml` / `Cargo.lock` (ワークスペースメンバー 2 件)。
**必ず全部同じ値**にする。CI の `version` ジョブ ([`.github/scripts/check-versions.sh`](../../../.github/scripts/check-versions.sh)) がずれを弾く。

利用者から見えるのは: ダッシュボードのフッタ (`web/src/components/dashboard-footer.tsx`)、Bot の `/help` の脚注、
管理コンソール `/admin` の「api のバージョン」。`/admin` の「イメージタグ」は**ビルド時の `sha-xxxxxxx` のまま**
(実行ファイルに焼き込まれるため。GHCR で `v3.x.y` を付け直しても変わらない) で、これは仕様。

## 手順

### 1. 出す内容を決める

```bash
git switch main && git pull --ff-only
git describe --tags --abbrev=0 --match 'v*'   # 前回のリリース (最初は何も出ない)
.github/scripts/release-notes.sh "$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null)"
```

出てきた更新履歴のエントリがそのままリリースノートになる。これを見て上げ幅 (major / minor / patch) を決める。
**利用者に見える変更が更新履歴に載っていない**ことに気づいたら、先にそれを追記する PR を出す (AGENTS.md のルール)。

### 2. リリース準備 PR

```bash
git switch -c claude/release-v3.1.0
.claude/skills/release/scripts/bump-version.sh minor    # または 3.1.0 のように直接指定
git add -A && git commit -m "リリース準備: v3.1.0"
git push -u origin claude/release-v3.1.0
gh pr create --title "リリース準備: v3.1.0" --milestone "v3 リリース" --body-file pr-body.md
```

- スクリプトが 4 か所を書き換える。更新履歴には**何も足さない** (エントリは各機能 PR で既に入っている。
  バージョンと日付の対応は GitHub Release 側に残る)
- この PR には**バージョン以外の変更を混ぜない**。CI が通ったらマージする (squash)

### 3. タグを打つ

```bash
git switch main && git pull --ff-only
.github/scripts/check-versions.sh          # 3.1.0 が出ることを確認
gh run list --workflow "Deploy staging" --limit 1    # main マージ後のイメージビルドが終わっているか
git tag -a v3.1.0 -m "v3.1.0" && git push origin v3.1.0
```

- **タグは必ず main のコミットに打つ** (release.yml が main に含まれるか確認して弾く)
- **"Deploy staging" の完了を待つ**。タグのコミットの `sha-xxxxxxx` イメージが GHCR に無いと release.yml は失敗する
  (待ってからタグを打ち直せばよい。打ち直すときは `git tag -d v3.1.0 && git push origin :refs/tags/v3.1.0` で消してから)
- Claude Code のクラウドセッションではカレントブランチ以外に push できない。タグ付けはユーザーに依頼するか、
  API で作る: `gh api repos/Shirataki2/DisCalendarV3-new/git/refs -f ref=refs/tags/v3.1.0 -f sha=$(git rev-parse main)`

release.yml がやること (数十秒):

1. タグの形式と、4 か所のバージョンがタグと一致するかを確認する
2. タグのコミットが main にあることを確認する
3. GHCR の `discalendar-{web,api,bot}:sha-xxxxxxx` に `v3.1.0` (プレリリースでなければ `latest` も) を **同じ digest のまま**付ける。
   再ビルドしないので、staging で動かしたイメージと必ず同一
4. 前回タグ以降の更新履歴からノートを作って GitHub Release を公開する

### 4. 本番に反映する

Actions → **"Deploy production"** → Run workflow → `image_tag` に **`v3.1.0`** を入れて実行する
(Environment `production` の承認が要る)。ホスト上の `.env` の `IMAGE_TAG` が書き換わり、`docker compose pull && up -d` が走って
db / api / web が healthy になるまで待つ。詳細は [README の「本番 (discalendar.app) へのデプロイ」](../../../README.md)。

反映後の確認:

```bash
gh run watch                     # デプロイの完了を待つ
curl -fsS https://discalendar.app/ > /dev/null && echo ok
```

- `/admin` の「稼働状況」で **api のバージョンが 3.1.0** になっていること (イメージタグは `sha-xxxxxxx` のままでよい)
- ダッシュボードのフッタ、Bot の `/help` の脚注も新しい版になっていること
- 変更した機能を実際に触る (Discord ログインが要るので、クラウドセッションからは確認できない。ユーザーに引き継ぐ)

### 5. ロールバック

**同じ "Deploy production" に前の版のタグを入れて実行するだけ** (`image_tag: v3.0.0`)。GitHub Release やバージョンは戻さない
(履歴として残す)。次に出す版は、修正を入れてから新しい番号 (`v3.1.1`) にする。

- DB マイグレーションが入った版から戻すときは注意。`api/migrations/` は前方互換 (旧 Bot も動く) 前提だが、
  新しいマイグレーションを適用済みの DB に古い api を当てて問題ないかを先に確かめる (AGENTS.md の P0)
- イメージのロールバックで直らないもの (データの問題など) は、README の「DB のバックアップと復元」を見る

## 落とし穴

- **タグを打つ前に "Deploy staging" を待つ**。イメージが無いとタグだけ残って release.yml が落ちる
- **バージョンだけ上げてタグを打たない**状態を作らない (フッタは 3.1.0 なのにリリースが無い、が起きる)。
  リリース準備 PR をマージしたらその日のうちにタグまで進める
- **タグは打ち直さない**のが原則。既に公開した `v3.1.0` を別のコミットに付け替えると、GHCR のタグと Release がずれる。
  間違えたら次の番号で出し直す (公開直後で誰も pull していないと確信できるときだけ、タグと Release を消してやり直す)
- `latest` は**最新の正式リリース**を指す (`compose.yaml` の `IMAGE_TAG` 既定値)。`release` ジョブの後に動く `latest` ジョブが、
  「今いちばん新しい公開済みの GitHub Release」を見て**そのリリースのイメージ**に向ける (自分のタグが最新でなくてもそこに収束する)。
  プレリリースと下書きは候補に入らない。古いタグの再実行でも巻き戻らず、失敗した run や打ち間違えたタグが残っていても止まらない
- **`v*` タグの作成はルールセットで制限しておく** (Settings → Rules → Rulesets → Tag ruleset で `v*` に "Restrict creations")。
  タグから起動するワークフローは**そのタグのコミットにある `release.yml`** が動くので、main に入っていない改変版を
  タグ付きで push されると write 権限のトークンで Release / GHCR を操作できてしまう。main のブランチ保護では塞げない
- 本番と staging は同じホストにいる。`.env` の `COMPOSE_PROJECT_NAME` / `WEB_PORT` を混同しない (README)

## 関連

- 機能 PR そのものの進め方は [`issue-driven-dev`](../issue-driven-dev/SKILL.md) スキル (更新履歴の追記もそこ)
- デプロイの仕組み: [`.github/workflows/deploy-staging.yml`](../../../.github/workflows/deploy-staging.yml) /
  [`deploy-production.yml`](../../../.github/workflows/deploy-production.yml) / [`.github/scripts/deploy.sh`](../../../.github/scripts/deploy.sh)
- 本番切替の経緯と当日の手順は #12
