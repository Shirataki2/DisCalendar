---
name: update-screenshots
description: DisCalendar の LP (トップページ) と使い方 (/docs) に貼っているスクリーンショットを撮り直す。「スクショを撮り直して」「画像が古い」「LP の画像を更新」「docs のスクショを新しい UI にして」のような依頼のほか、ダッシュボードの見た目・日時の表記・ナビゲーション・ダイアログのレイアウトを変える PR で画像が実物と食い違うようになったときに使う。撮影は E2E 環境 (api + Postgres + Next + Discord モック) を使う `pnpm shot` に一本化してあり、対象画像の一覧・撮り直しが要る条件・出来上がりの確認観点・既知の罠をここにまとめている。
---

# LP / 使い方のスクリーンショットを撮り直す

画像は `web/src/assets/lp/` と `web/src/assets/docs/` にあり、LP (`web/src/app/page.tsx`) と
使い方 (`web/src/content/docs/*.mdx`) が `import` して使っている。
撮影は E2E と同じ環境 (Discord をモックしたログイン済みのアプリ) で行い、**`pnpm shot` が 7 枚すべてを直接上書きする**。
手で撮ったり、`e2e/fixtures.ts` を書き換えて戻したりはしない。

## 対象の画像

| ファイル | 使っている場所 | 写っているもの | 撮り直しが要るのは |
|---|---|---|---|
| `lp/calendar.png` | LP のヒーロー / `docs/calendar.mdx` | 月表示のカレンダー全体 (サイドバー・ヘッダ・ツールバー込み) | カレンダー、日時の表記、ナビゲーション、配色を変えたとき |
| `lp/dialog.png` | LP / `docs/edit.mdx` | 予定の編集ダイアログ | 予定フォームの項目・レイアウトを変えたとき |
| `lp/settings.png` | LP / `docs/edit.mdx` | サーバー設定ダイアログ (restricted を選んだ状態) | サーバー設定の文言・項目を変えたとき |
| `docs/serverselect.png` | `docs/login.mdx` / `docs/invite.mdx` | サーバー選択 (参加済み + 招待できるサーバー) | サーバー選択やヘッダ・サイドバーを変えたとき |
| `docs/popover.png` | `docs/edit.mdx` | 予定のポップオーバー (背景に月表示が写る) | ポップオーバーの中身か、**カレンダーの日時の表記**を変えたとき |
| `docs/create-dialog.png` | `docs/calendar.mdx` | 予定の作成ダイアログ (入力済み) | 予定フォームを変えたとき |
| `docs/login.png` | `docs/login.mdx` | ログインページ | ログインページを変えたとき |

`alt` は画像の内容を説明しているので、**写るものを変えたら `alt` も直す** (例: ポップオーバーの `alt` は
「タイトルと日時、通知、説明と…」なので、説明のないサンプルにすると食い違う)。

## 手順

### 1. 前提を整える

- `web/` で作業する。ローカルの Postgres が動いていること (`pg_isready`)
- ポート 3100 (web) / 8180 (api) / 8191 (Discord モック) が空いていること。
  **撮影では web と api を必ず起動し直す**ので、前の `pnpm e2e` のサーバーが残っていると
  「port is already used」で止まる。残っていたら止める (`lsof -ti tcp:3100 | xargs kill`)
- api のビルドに時間をかけたくなければ、ビルド済みのバイナリを指す:
  `E2E_API_COMMAND=$(git rev-parse --show-toplevel)/target/debug/discalendar-api`

### 2. 撮る

```bash
pnpm shot
```

`web/e2e/screenshots/shots.spec.ts` が 8 個のテストとして走り (最初の 1 つはサンプルの予定を入れるだけ)、
7 枚を `src/assets/` に上書きする。失敗したテストの画像は更新されないので、`git status` で
7 枚そろっているかを見る。

### 3. 出来上がりを見る

**必ず全部を目視する** (テストは「画像が作られたこと」しか保証しない)。見るところ:

- 表記が新しくなっているか (時刻・日付・文言)。とくに `lp/calendar.png` と `docs/popover.png`
- E2E 用の名前 (`E2E Admin Guild`, `E2E User`) が写っていないか → 写っていたら 「よくある詰まりどころ」へ
- `next dev` の開発ツール (左下のロゴ) やフォーカスリングが写っていないか
- クリップの端で文字が不自然に切れていないか。切れていたら spec の `clipAround` の余白を調整して撮り直す
- ロゴ (`DISCALENDAR`) がフォールバックのフォントになっていないか (Uni Sans Heavy の太い字形か)
- 各画像の `alt` (LP は `page.tsx`、docs は `*.mdx`) と食い違っていないか

### 4. コミットする

画像と、変えたなら spec / `alt` を一緒にコミットする。PR の「動作確認」には
`pnpm shot` を流したことと、目視した観点を書く。
スクリーンショットの差し替えだけなら更新履歴 (`web/src/content/changelog.mdx`) には書かない
(利用者に見える機能の変更ではないため)。

## 中身を変えたいとき

| 変えたいもの | 直す場所 |
|---|---|
| 写っている予定 (名前・時刻・色・説明・通知) | `web/e2e/screenshots/sample-data.ts` |
| ユーザー名・サーバー名 | `web/e2e/fixtures.ts` の `displayName()` の第 2 引数と `SCREENSHOT_GUILDS` |
| 撮る画面・切り出す範囲・画面サイズ | `web/e2e/screenshots/shots.spec.ts` (`test.use` の `viewport` と `clipAround` の余白) |
| 画像を増やす / 減らす | 同 spec にテストを足す。出力先は `assetPath("lp/... .png")` |

サンプルの予定は**ブラウザから見た「今月」を基準に**作っている (`sampleEvents(today)`)。
絶対日付を書かないこと。いつ撮り直しても日付が古びず、月末が 28 日でもはみ出さないようにするため。

## 仕組み (なぜこうなっているか)

- 撮影は E2E の仕組みをそのまま使う。`playwright.config.ts` が api (Rust) と Next を立て、
  `e2e/global-setup.ts` が Discord API のモックを起動して DB を初期化し、ログイン済みの
  `storageState` を書く。ネットワークにも本物の Discord にも出ない
- `E2E_SCREENSHOT=1` (= `pnpm shot`) のときだけ変わるのは 3 つ:
  1. `e2e/fixtures.ts` の**表示名**が撮影用になる (`ゲーム部` など)。ID や権限は変えないので、
     アプリから見た条件はテストと同じ。サーバー選択の画像が寂しくならないよう、ギルドが 2 つ増える
  2. `e2e/env.ts` の **Discord モックのポートが 8190 → 8191** になる (後述のキャッシュ対策)
  3. ブラウザに `--lang=ja-JP` を渡す。`<input type="time">` のようなネイティブのフォーム部品の表記は
     ブラウザ UI の言語で決まり、日本語なら 24 時間制の `20:00`、既定の英語だと `08:00 PM` になる
     (`use.locale` は `Intl` にしか効かない)
- 撮影用のテストは `playwright.config.ts` の `testIgnore` で通常の `pnpm e2e` / CI から外してある
- 撮影のたびに DB は初期化されるので、`pnpm shot` を何度流しても同じ内容になる

## よくある詰まりどころ

- **画像に `E2E Admin Guild` / `E2E User` が写る**
  `web/src/lib/discord.ts` の `/users/@me/guilds` は `next: { revalidate: 60 }` でキャッシュされ、
  キーに URL が入る。撮影とテストで Discord モックのポートを分けているのはこれを混ぜないため。
  それでも写るなら、web を使い回していないか (`reuseExistingServer`) と、
  `E2E_DISCORD_MOCK_PORT` を自分で上書きしていないかを見る。最後の手段は `rm -rf .next/cache/fetch-cache`
- **`pnpm shot` が「port is already used」で止まる**
  前の `pnpm e2e` の web / api が残っている。撮影では使い回さない (向き先が違う) ので、止めてから流す
- **時刻が `08:00 PM` と出る** → ブラウザ UI の言語が日本語になっていない (上記 3.)
- **ロゴが違うフォントで写る** → `document.fonts.ready` を待たずに撮っている (`prepare()` を通す)
- **ダイアログが縦に切れる** → `test.use` の `viewport` の高さを増やす (ダイアログは `max-h` で
  スクロールするようになっているので、収まらないとスクロールバーが写る)

## やってはいけないこと

- 撮影のために `e2e/fixtures.ts` の名前を手で書き換えて戻す (戻し忘れると通常のテストが落ちる。
  差し替えたいなら `displayName()` の撮影用の値を直す)
- 撮影用のテストを `e2e/` 直下に置く (通常の `pnpm e2e` と CI で動いてしまう)
- サンプルの予定に絶対日付を書く (次に撮り直したときに空のカレンダーになる)
- 画像を手動でトリミング・加工する (次に撮る人が同じものを再現できなくなる)
