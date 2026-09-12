# プッシュ通知の構成と設定

[README に戻る](../README.md)

パスとコマンドは、特記がなければリポジトリルートを基準に記載する。

アカウントメニュー・ドロワーの「プッシュ通知」で端末を登録する。初期状態はオフで、全参加サーバー / 自分が作った予定 / オフをアカウントごとに保存する。
利用者への外部キーは、初回起動時に web のテーブルがまだ無ければ1秒ごとに再試行し、テーブル作成後に孤児を掃除して追加する。
表示設定のアカウント保存 (#178) とは独立した `user_push_settings` を使い、端末を全解除しても範囲は残す。

配信は **Bot が発火を判定して DB に記録し、API が送信する方式 (Issue の案 C)**。
Bot の判定窓・開始通知の設定・再起動時の5分の遡りをそのまま使い、Discord の通知先チャンネルが無くてもプッシュは配信する。
Bot は Better Auth のテーブルを読まず、API がアカウントと Discord ID を対応付ける。配信候補はサーバー・利用者ごとに60秒のキャッシュで所属を確認し、参加者だけに絞る。所属不明の結果もキャッシュして再照会を集約し、未展開の通知を循環して後続の展開を止めない。送信直前にはキャッシュを使わず所属を再確認する。
restricted は編集制限なので配信対象を狭めない。削除・時刻変更・通知設定の変更で不要になった通知は配信前に破棄する。

VAPID 鍵は環境ごとに一度生成する。Node.js 標準の暗号機能で生成できる (秘密鍵をコミット・ログに貼らない):

```sh
node --input-type=module -e 'import {createECDH} from "node:crypto"; const k=createECDH("prime256v1"); k.generateKeys(); console.log("NEXT_PUBLIC_VAPID_PUBLIC_KEY="+k.getPublicKey().toString("base64url")); console.log("VAPID_PRIVATE_KEY="+k.getPrivateKey().toString("base64url"));'
```

- ローカル: 公開鍵を `web/.env.local` の `NEXT_PUBLIC_VAPID_PUBLIC_KEY`、秘密鍵を `api/.env` の `VAPID_PRIVATE_KEY` に設定する。`VAPID_SUBJECT` は運営者の `mailto:連絡先` または HTTPS URL。
- compose: 同じ3変数をホストの `.env` に置く。公開鍵は web の `build.args`、秘密鍵と連絡先は api の実行環境にだけ渡される。
- GHCR: Repository variables の `VAPID_PUBLIC_KEY` (本番) / `STAGING_VAPID_PUBLIC_KEY` (staging) を web のビルド時に使う。ホストの秘密鍵と組が合う公開鍵を指定し、web を再ビルドしてから API / Bot と一緒にデプロイする。ビルド済み web は実行時に公開鍵を変えても反映されない。staging の変数が空なら本番鍵へフォールバックせず、staging の登録 UI は無効になる。
- API の秘密鍵・連絡先が両方空なら配信タスクを起動しない。一方だけの設定や不正な鍵は起動時に拒否する。公開鍵が空なら UI は「準備中」を表示する。
- 鍵を変更すると既存の購読には届かないため、各端末で登録し直す。
- ブラウザ・プッシュサービス側の購読更新 (`pushsubscriptionchange`) の自動同期は未対応。通知が届かなくなった場合は、設定画面から端末を登録し直す。

暗号化と VAPID には [web-push](https://docs.rs/web-push/0.11.0/web_push/) を使う。HTTP は既存の reqwest (rustls) で送信し、10秒でタイムアウト、リダイレクトを追わない。
暗号処理の ece が OpenSSL を使うため API の実行イメージに `libssl3` を含む。
送信先は Chrome / Firefox / Safari / Edge のプッシュサービスに限定し、任意の URL や内部ネットワークには送らない。
`push_subscriptions` は SQL コンソールの保護テーブルに含め、一覧 API も URL と鍵を返さない。

配信は1分のリースで取得し、外部通信中は DB 接続・行ロックを解放する。送信前に解除・オフ・予定変更を再確認し、結果の確定時には試行番号で所有権を確認する。
通知待ちは `push_outbox`、端末ごとの送信結果は `push_deliveries` に保存し、再起動後も引き継ぐ。
通知の展開と配信は独立して実行する。展開時の所属照会はサーバーをまたいでも全体で最大10件の並列処理に抑える。API は15秒ごとに最大100件を、最大10件の並列処理で配信し、失敗時は1分以上あけて最大5回試す。404 / 410 は端末の購読を削除し、5回の試行を使い切った通知が5件連続したら、その端末を停止する。
1時間を超えた通知は破棄し、記録は1日後に掃除する。停止中のプッシュサービス側の保持期間も1時間。
送信直後から DB の記録確定前にプロセスが落ちた場合は再送され得る。端末では同じ予定の tag で表示をまとめる。
戻す場合は API / Bot を止めてバックアップし、`api/rollback/20260907090000_drop_push_notifications.sql` を実行する (端末情報・範囲・通知待ちは削除され、再登録が必要)。

実機確認は `pnpm build` + `pnpm start` または HTTPS の staging で行う。dev は Service Worker を自動登録しない。
Android Chrome と [iOS 16.4以降のホーム画面アプリ](https://developer.apple.com/documentation/usernotifications/sending-web-push-notifications-in-web-apps-and-browsers) で、登録 → 近い時刻の予定を作成 → 事前 / 開始通知を受信 → タップで該当日へ遷移、を確認する。
続けて「自分が作った予定だけ」・オフ・サーバー退出・端末解除・ログアウト後の配信停止を確認する。
E2E はプッシュサービスだけを代替し、Chromium の通知許可、実際の Service Worker、購読 API の保存・解除までを確認する。実プッシュサービスへの到達は実機確認で補う。
