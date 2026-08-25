---
name: cleanup-workspace
description: DisCalendar リポジトリの後片付け。不要になった git worktree (.claude/worktrees/ 配下) と対応するローカルブランチの削除、Rust の target/ や web/node_modules・web/.next などビルド成果物の削除でディスクを空ける。「worktree を片付けて」「いらないブランチを消して」「target を消して」「ディスクが足りない / 容量を空けたい」「掃除して」「マージ済みのやつを消して」のような依頼のほか、PR がマージされた直後の後始末、ENOSPC や Docker ビルドの失敗、Mac の空き容量警告が出たときにも使う。squash マージのため git だけではマージ済みか判定できず PR の状態を gh で見る必要があり、その手順と安全チェック付きスクリプトをここに集約している。
---

# ワークスペースの後片付け (worktree / ブランチ / ビルド成果物)

Issue ごとに worktree を切る運用なので、マージ後に worktree と `target/` が残り続ける。
Mac のディスクは逼迫しがち (`target/` は worktree 1 つで数百 MB〜数 GB、`node_modules` 600 MB、`.next` 200 MB〜) で、
Docker ビルドが ENOSPC で落ちたこともある。
一方で、消してはいけないものを消すと取り返しがつかないので「まず見る → 判定 → 確認のうえ消す」の順を守る。

## 前提知識

- worktree は `.claude/worktrees/<name>`。ブランチ名は `claude/issue-N-<slug>` (このリポジトリの慣習)、`worktree-<name>` (EnterWorktree の既定)、`feat/...` など
- main は **squash マージ** → マージ後もブランチのコミットは `origin/main` に含まれない。`git branch --merged` / `merge-base` は使えない。
  マージ済みかは `gh pr list --head <branch> --state all` の `MERGED` で判定する。マージ後リモートブランチは自動削除されるので、`git fetch --prune` 後に `[gone]` になるのも手がかり
- **メインの checkout のセッションから実行する**。worktree セッション (EnterWorktree 中) は別パスへの `git -C` が拒否され、自分のいる worktree も消せない。
  worktree にいるなら先に `ExitWorktree` (`keep`) で戻る
- `tmp/` (旧実装 `tmp/DisCalendarV2/`) は git 管理外で**復元できない**。掃除の対象にしない (worktree 内に `tmp/` があれば `remove-worktree.sh` は `--force` でも止まる。外へ移してから)
- 他の Claude Code セッションやターミナル、dev サーバーが使っている worktree かもしれない (`lsof -a -d cwd -Fn | grep worktrees` で cwd にしているプロセスが分かる)。
  レポートと削除スクリプトはこれを検出して止まる。PR が OPEN のもの、最近更新されたものも消す前にユーザーに聞く
- `.env` 系や `*.pem` などの鍵 (git 管理外 = ignored) は `git status` の未コミットに数えられないが、worktree を消すと一緒に消える。
  レポートと削除スクリプトは、**再生成できる成果物以外**の ignored ファイルでメインの checkout に無い / 内容が違うものを列挙する。
  残すものは先にコピーして退避する (過去に `bot/.env` や compose 用のルート `.env` が worktree にしか無かった)
- 「再生成できる成果物」として除外しているもの (両スクリプトの `REGEN_RE`。増えたらここも直す):

  | 種類 | 対象 |
  |---|---|
  | ビルド・依存 | `target/` `node_modules/` `.next/` `out/` `build/` `dist/` `coverage/` `.vercel/` `.yarn/` `.turbo/` |
  | Playwright (`pnpm e2e` が作る) | `web/test-results/` (失敗時のスクリーンショット / trace) `web/playwright-report/` (HTML レポート) `web/e2e/.auth/` (ログイン済み storageState。中身は `user.json` だけで、テスト用のダミーセッション cookie) |
  | その他 | `.DS_Store` `next-env.d.ts` `.pnp*` `*.log` `*.tsbuildinfo` |

## 手順

### 1. 現状を見る (読み取り専用)

```bash
.claude/skills/cleanup-workspace/scripts/worktree-report.sh
```

`git fetch --prune` → 各 worktree の「ブランチ / PR の状態 / 未コミット数 / origin/main に無いコミット数 / upstream / 合計と target・node_modules・.next のサイズ /
直近 3 時間の更新 / cwd にしているプロセス / main に無い `.env` / 判定」と、worktree を持たないローカルブランチ、
ディスク・pnpm / cargo キャッシュ・Docker の使用量を Markdown の表で出す。実行時点のスナップショットなので、削除直前の再チェックは `remove-worktree.sh` に任せる。判定は次の 3 つ:

| 判定 | 意味 | 次にすること |
|---|---|---|
| 削除候補 | PR が MERGED / CLOSED で、未コミット・使用中プロセス・直近の更新・`.env` の危険がない | 2 へ |
| 要確認 | 未コミットがある / PR なし (作った直後かもしれない) / マージ済み PR の後にローカルコミットがある (`PR 後 +N`) / 他プロセスが使用中 / 直近に更新 / main に無い ignored ファイル (`.env` / 鍵 / `tmp/`) がある / 状態が取れない | 根拠を添えてユーザーに判断を仰ぐ |
| 作業中 | PR が OPEN | 残す (ユーザーが「この PR は閉じた」と言わない限り) |

### 2. ユーザーに提示して了解を得る

表をそのまま見せ、「削除候補」を一括で消してよいか、「要確認」はどうするかを聞く。要確認のものは根拠を添える:

```bash
git -C .claude/worktrees/<name> status --short          # 未コミットの中身
git log origin/main..<branch> --oneline                  # PR になっていないコミット
diff <path>/bot/.env bot/.env                            # main に無い / 違う .env の中身 (値は出力に貼らない)
```

未コミットや未 push のコミット、worktree にしかない `.env` を消すのは不可逆なので、ユーザーの明示的な了解なしに `--force` を使わない。
他プロセスが使用中のものは、そのセッションを閉じてもらってから (スクリプトは `--force` でも消さない)。

### 3. worktree とローカルブランチを消す

```bash
.claude/skills/cleanup-workspace/scripts/remove-worktree.sh <worktree 名 or パス>            # 安全チェック付き
.claude/skills/cleanup-workspace/scripts/remove-worktree.sh <worktree 名> --force            # 了解を得た「要確認」用
.claude/skills/cleanup-workspace/scripts/remove-worktree.sh <worktree 名> --keep-branch      # ブランチは残したいとき
```

スクリプトは未コミット・OPEN な PR・PR なしの先行コミットや直近の更新・**マージ済み PR の head より後のローカルコミット**・他プロセスの使用・main に無い `.env` があれば拒否し (exit 3、理由を表示)、問題なければ
`git worktree remove` → `git branch -D` → `git worktree prune` を行う。worktree を消すとその中の `target/` / `node_modules` / `.next` も一緒に消える (表の「合計」列でどれだけ空くか分かる。数 MB しかない worktree は整理目的、容量目的なら 4 へ)。
リモートブランチには触れない (残っていれば案内だけ出す。消すなら `git push origin --delete <branch>` をユーザー確認のうえで)。

worktree を持たないローカルブランチで「削除候補」のものは `git branch -D <branch>`。

### 4. ビルド成果物だけを消す (残す worktree / メインの checkout)

ディスクがまだ足りないときに、効果と再生成コストを伝えたうえで選ぶ:

| 対象 | コマンド | 再生成コスト |
|---|---|---|
| 残す worktree の `target/` | `(cd <path> && cargo clean)` または `rm -rf <path>/target` | 次回 `cargo build` でフルビルド (数分) |
| `web/.next` (dev / build キャッシュ) | `rm -rf <path>/web/.next` | 次回 `pnpm dev` / `pnpm build` で再生成。dev サーバーは先に止める |
| worktree の `web/node_modules` | `rm -rf <path>/web/node_modules` | `pnpm install` (pnpm ストアがあれば速い) |
| メインの checkout の `target/` (数 GB) | `cargo clean` | フルビルド数分 + IDE (rust-analyzer) の再解析。最後の手段 |
| Docker (OrbStack) | `docker system df` で見て `docker builder prune` / `docker image prune -a` (`-a` なしは dangling だけで、タグ付きの未使用イメージは残る) | イメージの再ビルド・再 pull (postgres / node など) |
| pnpm ストア (`~/Library/pnpm`) | `pnpm store prune` (未参照パッケージだけ消す) | 次回 install で再取得 |

メインの checkout の `node_modules` やホームの `~/.cargo/registry` は消さない (効果のわりに復旧が面倒)。

### 5. 結果を報告する

`git worktree list` と `git branch -vv` で残ったものを確認し、`df -h /` の前後で空いた容量を伝える。
残した「要確認」があれば、なぜ残したかと次にどうすれば消せるかを書く。

## やってはいけないこと

- 未コミット・未 push のある worktree を確認なしに消す (`--force` / `git worktree remove --force` / `rm -rf`)
- `.claude/worktrees/` を `rm -rf` で丸ごと消す (git の登録が残って `prune` が要るうえ、中身を見ずに消すことになる)
- リモートブランチの削除、PR のクローズ (どちらも外向きの操作。ユーザーの了解なしにやらない)
- `tmp/`、`.env` 系、`api/.sqlx/` / `bot/.sqlx/` を掃除対象にする
