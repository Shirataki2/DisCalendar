# Web の構成と実装

[README に戻る](../README.md)

パスとコマンドは、特記がなければリポジトリルートを基準に記載する。

## web と api のつなぎ方

- ブラウザからの API 呼び出しは同一オリジンの `/local/api/*` に投げ、`web/next.config.ts` の rewrites が
  `API_URL`（既定 `http://127.0.0.1:8080`）へプロキシする。Better Auth のセッション cookie がそのまま転送され、
  API 側がそれを検証する（旧実装の `@nuxtjs/proxy` と同じ構成）。
- Server Component からは `web/src/lib/api/server.ts` が cookie を付けて API を直接呼ぶ。
- クライアント側のデータ取得・更新は TanStack Query（`web/src/lib/query/`）。ドラッグ移動などは楽観的更新し、失敗時は元に戻す。
- API のエンドポイント定義は `web/src/lib/api/endpoints.ts`（Rust 側の `api/src/routes` と対応）。

## web の UI 部品

- UI 部品は shadcn/ui（`web/components.json`、現行の既定どおり Base UI ベース）。生成物は `web/src/components/ui/` に置き、
  `pnpm dlx shadcn@latest add <name>` で追加する。配色はダーク固定（`globals.css` の `.dark` を `<html class="dark">` で常時適用）。
- 予定の作成・編集ダイアログは `web/src/components/event-form-dialog.tsx`（React Hook Form + Zod）。
  スキーマと API との変換は `web/src/lib/event-form.ts` にあり、上限値（タイトル 32 文字、説明 1000 文字、通知 10 件）は
  `api/src/models/events.rs` と揃える。
- 予定をクリックすると概要ポップオーバー（`event-popover.tsx`、旧 SimpleEdit.vue 相当）から編集・削除できる。
- サーバー設定ダイアログは `web/src/components/guild-settings-dialog.tsx`（旧 ServerSetting.vue 相当）。restricted
  （予定の編集を管理権限を持つユーザーに限定）の切り替えと、Discord 側で権限を変えた後の「再読込」ができる。
  ギルド設定と自分の権限は `dashboard/[id]/page.tsx`（RSC）が取得して TanStack Query に hydrate し、
  `guild-dashboard.tsx` がそこから編集可否を求めるので、保存するとカレンダーの編集可否がその場で切り替わる。
- ダッシュボード（`/dashboard`、`/dashboard/[id]`）の枠は `app/dashboard/layout.tsx` → `dashboard-shell.tsx`
  （アプリバー + ナビゲーションドロワー。PC では常設サイドバーで開閉を cookie `dashboard_sidebar` に覚え、スマホでは Sheet）、
  `dashboard-nav.tsx`（リンク一覧。旧 NavDrawer.vue と同じ並び）、`dashboard-footer.tsx`（固定フッタ。バージョンは
  `web/package.json` の `version` で api / bot の Cargo.toml と揃える）、`user-menu.tsx`（アバターのドロップダウン）。

## web の使い方ページ (docs)

- `/docs/<slug>` は `@next/mdx` で静的生成する（旧 `@nuxt/content` の 7 ページと同じ URL）。本文は `web/src/content/docs/<slug>.mdx`、
  ページの一覧と並び順（サイドナビ・前後リンク・`generateStaticParams`）は `web/src/lib/docs.ts`、
  描画は `web/src/app/docs/[slug]/page.tsx`（`dynamicParams = false`）と `web/src/app/docs/layout.tsx`。
- MDX の見出し・リンクなどの見た目は `web/src/mdx-components.tsx`、本文用の部品（スクリーンショット、ボタン、注意書き、手順、コマンドカード）は
  `web/src/components/docs/`。表を使うため `remark-gfm` を入れている（Turbopack にはパッケージ名の文字列で渡す）。
- スクリーンショットは `web/src/assets/docs/`（`next/image` の静的 import）。LP と共用のものは `web/src/assets/lp/`。
  ページを増やすときは `.mdx` を足して `DOC_PAGES` に 1 行追加する。

## web の規約ページ (利用規約 / プライバシーポリシー)

- `/support/tos` と `/support/privacy` は docs と同じ `@next/mdx` で静的生成する（旧実装と同じ URL）。本文は
  `web/src/content/support/<slug>.mdx`、タイトル・要約・最終更新日は `web/src/lib/support.ts` の `SUPPORT_PAGES`、
  描画は `web/src/app/support/[slug]/page.tsx`（`dynamicParams = false`）と `web/src/app/support/layout.tsx`。
- 導線はヘッダ・フッタ（`site-header.tsx` / `site-footer.tsx`）とログイン画面（`app/login/page.tsx`）から。
- プライバシーポリシーは実装に合わせて書いてあるので、取得する情報（Better Auth のスコープ、保存するカラム）や
  Cookie の使い方、アクセス解析の導入を変えたときは本文と `updatedAt` を更新する。

## web の PWA (manifest / Service Worker)

- マニフェストは `web/src/app/manifest.ts`（`/manifest.webmanifest`。名前・説明・`theme_color` は `web/src/lib/site.ts`）。
  アイコンは `web/public/icons/`（通常の 192 / 512 と、Android の adaptive icon 用に余白を付けた `icon-maskable-*`）。
  iOS のホーム画面用アイコンは `web/src/app/apple-icon.png`。
- Service Worker は [Serwist](https://serwist.pages.dev/) の `@serwist/turbopack`（`@serwist/next` は webpack プラグインなので
  Turbopack の Next 16 では使えない）。本体は `web/src/app/sw.ts`、配信は `web/src/app/serwist/[path]/route.ts`
  （`createSerwistRoute` が `sw.ts` を esbuild で束ね、`next build` 時に `/serwist/sw.js` として静的生成する。
  `.next/static/` の JS / CSS の一覧が precache として埋め込まれる）。登録は `web/src/components/service-worker-provider.tsx`。
- キャッシュするのはハッシュ付きの `/_next/static/*`（precache の JS / CSS と、使われたときに残すフォント・画像）だけ。
  ページ（HTML / RSC）・Better Auth（`/api/auth/*`）・Rust API（`/local/api/*`）は Service Worker が関与しないので、
  認証付きのレスポンスが Cache Storage に残ることはない。オフライン用のフォールバックページは無い。
- `next dev` では登録しない（同じオリジンに残っている登録とキャッシュも消す）。挙動を確かめるときは本番ビルドで:
  `pnpm build && pnpm start -p 3100` → DevTools の Application タブ（Manifest / Service workers / Cache storage）。
  Lighthouse の PWA カテゴリは v12 で廃止されたため、`pnpm dlx lighthouse@11 http://localhost:3100/ --only-categories=pwa` で見る。
- `/serwist/*` には `next.config.ts` の `headers()` で `Cache-Control: no-cache` を付けている
  （静的生成されたルートに Next が付ける `s-maxage=31536000` のままだと、CDN に古い `sw.js` が残ってデプロイ後も更新されない）。

## サイト設定と外部リンク

LP の「BOT を導入する」は `/invite` (Route Handler) が `DISCORD_BOT_INVITE_URL` (未設定なら `DISCORD_CLIENT_ID` から組み立て) へリダイレクトする。
支援ページ (`/donation`) の「支援する」も同じ方式で、`/donation/checkout` (Route Handler) が
`STRIPE_DONATION_PAYMENT_LINK_URL` (Stripe Payment Links の URL。未設定なら 503「準備中」) へリダイレクトする。
サイト名・説明・公開 URL・外部リンクなどの定数は `web/src/lib/site.ts` にまとめてある。

## ルート構成 (web)

| パス | 内容 |
|---|---|
| `/` | ランディング |
| `/login` | Discord ログイン |
| `/dashboard` | サーバー選択（Bot 参加済み / 招待可能なサーバー） |
| `/dashboard/[id]` | ギルドごとのカレンダー |
| `/admin` | 管理コンソール（`ADMIN_DISCORD_USER_IDS` のユーザーのみ。それ以外は 404）: 件数・DB / マイグレーション・ビルド情報の概要 |
| `/admin/guilds`, `/admin/guilds/[id]` | 管理コンソール: 全ギルドの一覧・検索と Discord との差分検出、ギルドごとの予定の閲覧・編集 |
| `/admin/sql` | 管理コンソール: 読み取り専用 SQL コンソール (結果の表・実行履歴) と定型操作 |
| `/admin/users` | 管理コンソール: ユーザーの検索とセッションの確認・強制ログアウト |
| `/admin/audit-logs` | 管理コンソール: 監査ログの閲覧 (操作の種類・実行者で絞り込み) |
| `/api/auth/*` | Better Auth（OAuth コールバック含む） |
| `/manifest.webmanifest`, `/serwist/sw.js` | PWA のマニフェストと Service Worker（`app/manifest.ts`, `app/sw.ts`） |
| `/local/api/*` | Rust API へのプロキシ（rewrites） |
