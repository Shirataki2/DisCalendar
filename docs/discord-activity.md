# Discord の Activity (#350)

DisCalendar を Discord の Activity (Embedded App SDK) として登録し、ボイスチャンネル・テキストチャンネルから
起動したサーバーのカレンダーを Discord の中で開けるようにする。利用者向けの説明は使い方の「Discord の中で開く」(`/docs/activity`)。

初版の範囲は「カレンダーを見て予定を作る・編集する」まで。同じ Activity にいる人どうしの画面の同期は扱わない。

## 仕組み

```text
Discord クライアント
  └─ iframe: https://<client_id>.discordsays.com/?frame_id=...&instance_id=...   (Discord のプロキシ)
       │  URL Mapping: / → discalendar.app
       ▼
  web (Next.js)
    proxy.ts: frame_id / instance_id 付きの `/` を `/activity` へ rewrite (URL は変えない)
    /activity: ActivityApp (クライアント描画)
      1. DiscordSDK.ready() → commands.authorize({ scope: identify email guilds, prompt: none }) → code
      2. POST /api/auth/discord-activity/sign-in { code }
           → Discord でトークン交換 → /users/@me → 通常ログインと同じ規則でユーザー・account を作成/更新しセッションを作る
           → 署名付きセッショントークン `<token>.<HMAC>` を JSON で返す (cookie は付けない)
      3. 以降の api 呼び出し (/local/api/*) に `Authorization: Bearer <署名付きトークン>` を付ける
      4. sdk.guildId のカレンダー (GuildDashboard) を表示
  api (Rust)
    AuthUser が cookie と同じ検証で Bearer を受け付ける (api/src/auth.rs。この機能のための変更なし)
```

- 実装: `web/src/lib/activity/` (Better Auth プラグイン・署名・リンク判定)、`web/src/components/activity/activity-app.tsx`、
  `web/src/app/activity/`、`web/src/proxy.ts`、`web/src/lib/api/client.ts` (`setApiAuthToken`)
- 権限の判定 (`GuildMember`、`can_manage_server`、restricted モード、`guild_id` での絞り込み) は通常の Web と同じく api が行う。
  Activity 用に弱めたチェックは無い

### 認証方式を cookie ではなく Bearer にした理由

- Activity は `discord.com` を親に持つ `discordsays.com` の iframe で動き、web の cookie は第三者 cookie になる。
  Better Auth の既定の cookie (`SameSite=Lax`、Partitioned なし) は届かず、`SameSite=None; Partitioned` にしても
  iOS の WebView などで送られない環境が残る。通常の Web の cookie 属性を変えたくない
- api はもともと `Authorization: Bearer <cookie と同じ署名付き値>` を受け付けており、Next の rewrites は Authorization ヘッダーも転送する。
  web 側で署名付きトークンを発行してメモリに持つだけで済み、api と DB スキーマの変更が要らない
- トークンはブラウザのメモリにだけ置き (localStorage などに保存しない)、Activity を起動するたびに authorize からやり直す。
  2 回目以降は Discord の同意画面は出ない (`prompt: none`)
- セッションは通常ログインと同じ Better Auth の `session` 行で、有効期限・失効 (ログアウト・管理コンソールからの失効) も同じ扱い

## Discord Developer Portal の設定

本番・staging それぞれの Discord アプリ (Bot と同じアプリ) で行う。web と同じ `DISCORD_CLIENT_ID` / `DISCORD_CLIENT_SECRET` を使い、
web 側の環境変数の追加は不要。

1. **Activities → Settings**: Activities を有効にする (Enable Activities)。対応プラットフォーム (Web / iOS / Android) を選ぶ
2. **Activities → URL Mappings**:

   | PREFIX | TARGET |
   | --- | --- |
   | `/` (Root Mapping) | `discalendar.app` (staging は `staging.discalendar.app`) |
   | `/discord-cdn` | `cdn.discordapp.com` |

   api への通信は web の `/local/api/*` (rewrites) を通るので、api 用のマッピングは要らない。
   `/discord-cdn` はサーバーアイコンなどの画像用 (`patchUrlMappings` で書き換える)
3. **OAuth2**: Redirects の追加は不要 (Activity の authorize は redirect_uri を使わない)。scope は `identify email guilds` を求める
4. **Installation / Entry Point**: Activities を有効にすると、Discord が Entry Point コマンド (`/launch` など、type 4) を自動で作る。
   Bot の `@DisCalendar register` → 「全サーバーに登録 / 削除」は、このコマンドを残したまま上書きする
   (`bot/src/commands/register.rs`。含めずに一括上書きすると Discord が 50240 で拒否する)
5. 公開前は Developer Portal の **App Testers** に入れた人だけが起動できる。公開するには Activity の審査/公開設定を行う

## 確認手順 (staging)

1. staging の Discord アプリで上の設定をし、staging にデプロイする
2. Bot がいるサーバーのボイスチャンネル / テキストチャンネルから Activity を起動する (デスクトップ・モバイルの両方)
   - 初回は同意画面が出て、そのサーバーのカレンダーが開く
   - 予定の作成・編集・削除ができる。restricted モードで権限のない人は編集できない
3. Bot のいないサーバー・DM から起動すると案内が出る
4. 使い方・共有リンク・予定の URL などを押すと、Discord の確認を挟んでブラウザで開く
5. `@DisCalendar register` → 「全サーバーに登録」が成功し、Entry Point コマンドが残っている

ブラウザで `https://<web>/?frame_id=x&instance_id=y` を開くと Activity の画面 (読み込み中のまま) になり、
`/activity` を直接開くと「Discord のアクティビティ専用」の案内が出る。E2E (`web/e2e/activity.spec.ts`) はこの振り分けと、
Discord モックでのコード交換 → Bearer で api を呼ぶところまでを確かめる。

## 既知の制約

- 画面の同期はしない (各自の画面で操作し、保存した内容は再読み込みで反映される)
- Discord の CSP により、URL マッピングに無いオリジンへの通信は止まる
  - 添付ファイルのアップロード (R2 の署名付き URL へ直接 PUT) は動かない可能性が高い。ダウンロードはブラウザで開く
  - Sentry (ブラウザ側) への送信は届かない
- `navigator.clipboard` が iframe で使えない環境では、共有リンクのコピーに失敗する (画面の URL 欄から手でコピーする)
- 日程調整・すべての予定・サーバーの切り替えなど、Activity 以外の画面へのリンクはブラウザで開く
- Activity では Service Worker を登録しない (プッシュ通知の設定は Activity からはできない)
- Better Auth のレート制限は IP 単位。Discord のプロキシ経由で IP がまとまる場合に制限に掛からないかは、実機で確認する
