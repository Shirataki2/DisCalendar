# 予定変更の Webhook

[README に戻る](../README.md)

パスとコマンドは、特記がなければリポジトリルートを基準に記載する。

サーバー管理者が設定画面で最大 5 件の送信先を登録できる。利用方法・本文・署名検証は
[使い方](../web/src/content/docs/webhooks.mdx) を参照。
api の通常操作・管理コンソールと bot の作成処理が、予定変更と同じトランザクションで
`guild_webhook_outbox` に予約し、api の8ワーカーが専用 DB プール (最大8接続) で並列配信する。同じ Webhook 内は直列化し、
最大 3 回 (5 秒・30 秒後)、1 回 10 秒で試す。試行が 10 回連続で失敗すると無効化して
`webhook disabled after consecutive failures` を WARN に残す。専用の Grafana アラートは追加していない。

新規の 3 テーブルだけを追加する。api を起動してマイグレーションを適用してから bot を更新する。
戻す手順は `api/rollback/20260913000000_drop_guild_webhooks.sql` を参照 (登録情報・未配信データは失われる)。
署名シークレットと URL は DB に平文で保管するため、`guild_webhooks` は SQL コンソールの参照禁止対象。
`api/src/outbound_http.rs` は DNS 検証と接続先の固定をまとめた、外部 HTTP 接続の共通処理。
外部 ICS 購読 (#173) を追加するときもこの判定を利用する。

Webhook の E2E は `webhook-e2e` Cargo feature を使う (`pnpm e2e` の既定と CI の E2E ビルドで有効)。
このビルドだけが `webhook.test` をローカル受信モックに解決する。
通常・本番の Docker ビルドでは有効にしない。E2E の API バイナリを手動指定する場合は
`cargo build -p discalendar-api --features webhook-e2e` で用意する。
