# DisCalendar モダナイズ: 技術選定

- 作成日: 2026-08-20
- 対象: https://discalendar.app で稼働中の DisCalendar（現行コード: `tmp/DisCalendarV2`）の Web フロントエンドを、UX を保ったままモダナイズする
- スコープ: フロントエンドの周辺ライブラリ選定（API / Bot の Rust 側は本ドキュメントでは対象外。末尾に注記のみ）

## 1. 現行構成の調査結果

| 領域 | 現行 | 状況 |
|---|---|---|
| フレームワーク | Nuxt 2 (Vue 2) + `nuxt-property-decorator`（クラスコンポーネント） | Vue 2 は EOL。Nuxt 2 も EOL |
| UI | Vuetify 2（Material Design、ダークテーマ固定） | Vuetify 2 は EOL。`v-calendar` / `v-date-picker` / `v-time-picker` / `v-dialog` を多用 |
| カレンダー | Vuetify `v-calendar` | month / week / day / 4day 表示、ドラッグ移動・下端リサイズ・右クリック簡易編集・現在時刻ライン。**アプリの UX の核** |
| 状態管理 | Vuex + `vuex-module-decorators` | auth / calendar の 2 モジュール。calendar は実質イベントバス用途 |
| HTTP | axios + `@nuxtjs/proxy`（`/discord/api` → Discord、`/local/api` → Rust API） | プロキシ構成自体は妥当 |
| 日付 | moment（`@nuxtjs/moment`） | moment はメンテナンスモード宣言済み |
| 認証 | 自前実装の Discord OAuth2 code flow | **`client_secret` がクライアントに露出**（`callback.vue` がブラウザからトークン交換）、トークンを非 HttpOnly の cookie に `secure: false` で平文保存 |
| Discord 型 | `discord.js` v13（web の dependencies に同梱）+ 手書き型 + `json-bigint` | ブラウザバンドルに discord.js は過剰。Snowflake ID を bigint で扱うために `json-bigint` が必要になっている |
| コンテンツ | `@nuxt/content` v1（docs 7 ページ + 規約 2 ページの Markdown） | 量は少ない |
| i18n | `nuxt-i18n`（package.json にあるが **nuxt.config に未設定** = 実質未使用、日本語のみ） | 移行時に持ち越す必要なし |
| PWA / GA | `@nuxtjs/pwa`（workbox）、`@nuxtjs/google-analytics`（UA 世代） | GA は Universal Analytics 前提で既に動いていない可能性が高い |
| テスト | ava + `@vue/test-utils` | `test` スクリプトは `echo ok` |
| パッケージ管理 | yarn v1 | メンテナンスモード |

### 保つべき UX（調査から抽出）

1. Google カレンダー風の操作感: 月/週/日/4日ビュー、ドラッグで予定移動、下端ドラッグで時間延長、15 分スナップ、現在時刻の赤ライン、右クリック（タッチ長押し）で簡易編集ポップアップ、ダイアログでの新規作成/詳細編集
2. ダークテーマ固定のミニマルな見た目（ロゴは Courier Prime）
3. Discord ログイン → ギルド選択 → ギルドごとのカレンダー、権限（administrator / manage_guild 等）による編集制限（restricted モード）
4. 日本語 UI、docs / 利用規約 / プライバシーポリシーの静的ページ

## 2. 方針: React 化の判断

**React (Next.js) への移行を推奨する。**

- Vue 2 → Vue 3（Nuxt 4）移行でも、クラスコンポーネント・Vuex・Vuetify 2 はすべて書き直しになるため、実質フルリライト。コストが同等なら、エコシステム（カレンダー、認証、型定義、AI 支援含む）が最も厚い React に寄せる方が回収が大きい
- UX の核はカレンダーの操作感であり、これはフレームワークではなくカレンダーライブラリ（FullCalendar）が担保する

## 3. 選定結果

| 領域 | 採用 | 現行からの置換 | 主な理由 |
|---|---|---|---|
| フレームワーク | **Next.js 16 (App Router) + React 19 + TypeScript (strict)** | Nuxt 2 | LP/docs の静的生成 + 認証付きダッシュボード + BFF（後述の認証・プロキシ）を 1 つで賄える。Docker standalone 出力で現行の compose 運用にそのまま載る |
| カレンダー | **FullCalendar v7**（`@fullcalendar/react` 7.x + `temporal-polyfill`） | Vuetify `v-calendar` | month/week/day ビュー、ドラッグ移動・リサイズ・スナップ・現在時刻インジケータが MIT のコア機能で揃い、現行 UX をほぼ 1:1 で再現できる（PoC で検証済み）。Schedule-X はドラッグ&ドロップが Premium（有料）化、react-big-calendar は機能・保守で見劣り。v7 ではプラグインが `@fullcalendar/react/daygrid` 等のサブパスに統合され、v6 の個別パッケージ（`@fullcalendar/daygrid` 等）は使わない |
| UI | **Tailwind CSS v4 + shadcn/ui**（選定時は Radix ベース。2026-07 以降 shadcn の既定が Base UI になったため、導入時はその既定に従い Base UI ベースで生成） | Vuetify 2 | ダークテーマ前提のカスタムデザインを最小コストで。Dialog / Popover / Form 部品を置換。Material 感の完全維持が必要なら MUI v7 が代替（ただしバンドル増） |
| 日時ピッカー | shadcn/ui Calendar（react-day-picker、導入時点で v10）+ ネイティブ `<input type="time">` | `v-date-picker` / `v-time-picker` | 現行フォームの必要十分をカバー。作り込みが辛ければ MUI X Date/Time Picker（MIT 版）を部分導入する逃げ道あり |
| サーバー状態 | **TanStack Query v5** | Vuex + 手動 axios | イベントの取得/更新/削除・楽観的更新（ドラッグ移動の即時反映→保存失敗時ロールバック）が現行の手書きロジックをそのまま置換 |
| クライアント状態 | **Zustand v5**（最小限） | Vuex mutation のイベントバス的用法 | 表示タイプ・選択日などツールバー⇔カレンダー間の少量の共有状態のみ。大半は TanStack Query と URL に寄せる |
| HTTP | **ネイティブ fetch**（薄いラッパー）+ Next.js `rewrites` でプロキシ維持 | axios + `@nuxtjs/proxy` | 依存削減。`/local/api` → Rust API のプロキシは rewrites で同等構成。将来 Rust 側に utoipa で OpenAPI を導入したら `openapi-fetch` で型安全化 |
| 日付 | **date-fns v4**（+ 必要なら `@date-fns/tz`） | moment | ツリーシェイク可能・関数型・FullCalendar と両立。タイムゾーンは現状 JST 固定運用なので薄く |
| 認証 | **Better Auth**（Discord ソーシャルログイン、セッションは Postgres） | 自前 OAuth（client_secret 露出） | Auth.js は 2026 年からメンテナンスモードで、公式が新規プロジェクトに Better Auth を推奨。トークン交換をサーバー側に移し、セッションは HttpOnly cookie に。**現行の client_secret 露出・平文トークン cookie を解消**。Discord API 呼び出し（guild 一覧等）はサーバー側 Route Handler で代行 |
| フォーム | **React Hook Form + Zod v4** | `v-form` の rules | タイトル 32 文字制限などのバリデーションを型付きスキーマに集約。API 境界の型検証にも Zod を再利用 |
| Discord 型/権限 | **discord-api-types**（型のみ）+ 権限は BigInt ビット演算の小さな utility | discord.js v13 + 手書き型 + json-bigint | 実行コード不要で型だけ取れる。Snowflake は公式 API と同じく **string で扱い、`json-bigint` を廃止**（Rust API 側も u64 を文字列でシリアライズするよう変更を推奨） |
| docs/規約 | **MDX（@next/mdx）** | `@nuxt/content` v1 | 対象は 9 ファイルの静的 Markdown なので専用 CMS 不要。docs サイトを拡充したくなったら Fumadocs |
| i18n | 当面導入しない（必要になったら next-intl） | nuxt-i18n（未使用） | 現行でも実質未使用。負債を持ち越さない |
| PWA | **Serwist**（優先度低、後回し可） | `@nuxtjs/pwa` | next-pwa の後継。オフラインキャッシュ要件は薄いため最後でよい |
| アナリティクス | GA4 を **@next/third-parties** で | `@nuxtjs/google-analytics`（UA） | UA は廃止済みのため GA4 移行が必須 |
| テスト | **Vitest + Testing Library + Playwright（E2E）** | ava（実質未整備） | カレンダー操作（ドラッグ等）は Playwright の E2E が最も費用対効果が高い |
| Lint/Format | **Biome v2** | ESLint + babel-eslint | 単一ツール・高速。プラグイン資産が必要になったら ESLint 9 flat + Prettier に切替可能 |
| パッケージ管理 | **pnpm**（Node 22 LTS） | yarn v1 | 標準的な現行選択 |

## 4. 移行時の注意点

1. **セキュリティ修正が最優先の副産物**: 現行は `CLIENT_SECRET` が Nuxt の `env` 経由でブラウザに渡り、トークン交換もブラウザで行っている。Better Auth 導入で構造ごと解消する（旧サイトにも早期の対処を推奨）
2. **Snowflake ID の扱いを string に統一**: `json-bigint` 依存は Rust API が u64 を数値で返すことが原因。新 API レスポンスでは Discord 公式同様 string にする（`serde` で `#[serde(with = "...")]` あるいは newtype）
3. **FullCalendar v7 の API 差分**: v6 とパッケージ構成・API が変わっている（プラグインは `@fullcalendar/react/*` サブパス、CSS は `skeleton.css` + テーマ CSS を明示 import、ダークモードは `data-color-scheme="dark"` 属性、ビューのボタンラベルは `views.*.buttonText` ではなく `buttons: { <viewName>: { text } }`、内部日時は Temporal ベースだが `EventApi` は従来どおり `Date` を返す）。Web 上の情報は v6 前提が多いので注意。`nowIndicator`、`snapDuration: '00:15'`、カスタム 4 日ビュー等は設定のみで現行挙動を再現できることを PoC で確認済み。ダークテーマの色は CSS 変数（`--fc-classic-*`）で調整する
4. **restricted モード**: 権限判定（administrator / manage_guild / manage_messages / manage_roles）はサーバー側（Route Handler または Rust API）で行い、クライアントは表示制御のみにする
5. **モバイル UX**: 現行はタッチ操作（`touchend` で簡易編集）に対応している。FullCalendar の `longPressDelay` 系の設定で同等の操作感を確認すること

## 5. 推奨する進め方

1. Next.js スキャフォールド + Better Auth で Discord ログイン〜ギルド選択まで（認証の縦串を最初に通す）
2. FullCalendar でカレンダー画面の PoC: 月/週/日ビュー、ドラッグ移動・リサイズ・現在時刻ライン → **UX 再現度をここで検証**（最大リスクを先に潰す）
3. TanStack Query + rewrites で Rust API 接続、イベント CRUD
4. 新規作成/編集ダイアログ（React Hook Form + Zod）、サーバー設定ダイアログ
5. LP / docs / 規約ページ（MDX）、GA4
6. Playwright E2E、PWA（Serwist）、仕上げ

## 6. スコープ外への注記（Rust 側）

- `api`: actix-web 4.0.0-beta.9 → stable 4.x、sqlx 0.5 → 0.8 への更新が別途必要。utoipa による OpenAPI 出力を入れるとフロントの型生成（openapi-typescript / openapi-fetch）と接続できる
- `bot`: poise 0.3 / serenity 0.10 も同様に要更新（Discord API 側の仕様変化に注意）

## 参考（2026-08 時点の確認ソース）

- Next.js 16.3 が最新安定版: https://nextjs.org/blog/next-16-3
- Auth.js はメンテナンスモードとなり Better Auth へ合流、新規は Better Auth 推奨: https://github.com/nextauthjs/next-auth/discussions/13252
- Schedule-X のドラッグ&ドロップ/リサイズは Premium 化: https://blog.logrocket.com/best-react-scheduler-component-libraries/
- FullCalendar（MIT コアで drag & drop / resize / nowIndicator）: https://github.com/fullcalendar/fullcalendar
