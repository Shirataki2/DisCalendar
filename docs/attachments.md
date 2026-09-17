# 予定の添付ファイル

予定の編集権限があるメンバーがJPEG・PNG・WebP・PDFを添付できる。閲覧は予定と同じサーバーのメンバーに限定する。
1ファイル10MiB、1予定10件、サーバー合計1GiBまで。未確定の予約も件数・容量に数える。
容量は元ファイルの論理サイズで、一時コピーと削除待ちによるR2の一時的な使用量は別に発生する。

## R2の準備（開発・staging・本番）

1. `infra/terraform/terraform.tfvars.example` の `attachment_buckets` を参考に、環境別のバケット名とWebの正確なオリジンを設定する。
   開発例は `http://localhost:3000`。stagingと本番にはそれぞれのHTTPSオリジンだけを設定する。
2. 通常のTerraform手順でplanを確認し、バケット・CORS・一時ファイルのライフサイクルを適用する。
   Standard、非公開のまま使用し、`r2.dev`や公開カスタムドメインを有効にしない。
   バックアップ用バケット・ロック・認証情報は流用しない。確定済みファイルに期限は設定しない。
3. R2管理画面で環境ごとに **Object Read & Write** のAPIトークンを作り、対象をその添付バケットだけに限定する。
   バケット設定を変更できるAdmin権限は不要。トークン発行をTerraformで管理するとstateに秘密が残るため、ここでは発行しない。
4. APIの環境変数に以下を設定する。ローカルは `api/.env`、Composeは各環境のホストにある `.env` に置く。
   WebやBotには渡さず、GitHubのビルド引数にも入れない。

```dotenv
ATTACHMENTS_S3_ENDPOINT=https://<account-id>.r2.cloudflarestorage.com
ATTACHMENTS_BUCKET=<環境専用バケット名>
ATTACHMENTS_ACCESS_KEY_ID=<そのバケット専用のアクセスキーID>
ATTACHMENTS_SECRET_ACCESS_KEY=<シークレットアクセスキー>
```

4項目とも未設定なら添付機能だけ無効になる。一部だけの設定はAPIの起動エラー。
バケットを先に用意し、APIの新規マイグレーションを適用してからWebを更新する。既存予定のデータ変換はない。
本番反映は通常のリリース手順で行う。

## アップロード・配信

予約APIが予定の存在・認可と件数・容量を確認し、5分有効の署名付きPUT URLを返す。
ブラウザはContent-TypeとIf-None-Matchを指定し、Fileをそのまま送る。
Content-Lengthはブラウザが付けるためJavaScriptから指定しないが、署名には含めてサイズを固定する。

完了APIはHEADでサイズ・種類、Range GETで先頭のマジックバイトを検証する。
検証はウイルススキャンではない。PDFは埋め込み表示せず、必ずダウンロードする。
検証したETagを条件に別キーへコピーし、DBを確定する。署名付きPUTの再使用でも確定ファイルは書き換わらない。
ファイル名はキーに使わず、ダウンロードヘッダにはUTF-8でエンコードした名前を設定する。

閲覧・ダウンロードの署名付きGET URLも5分有効。URLを知る人は失効するまで利用でき、発行後の権限変更では即時失効しない。
URLとファイルは`no-store`で返し、Service Worker・画像最適化にも保存しない。
ダウンロード済みのコピーは回収できない。URLをログ・問い合わせ文・スクリーンショットに含めない。

API:

- `GET /guilds/{guild_id}/attachments`: 有効状態・上限・予約込み使用量
- `GET /events/{guild_id}/{event_id}/attachments`: 確定済み一覧
- `POST /events/{guild_id}/{event_id}/attachments`: ファイル名・サイズ・種類で予約
- `POST /events/{guild_id}/{event_id}/attachments/{attachment_id}/complete`: 検証・確定（再実行可能）
- `POST /events/{guild_id}/{event_id}/attachments/{attachment_id}/url?preview=true`: 画像プレビューURL（省略時はダウンロード）
- `DELETE /events/{guild_id}/{event_id}/attachments/{attachment_id}`: 添付・未確定予約の削除

公開共有リンク・iCal・MCP・Discord通知には添付を含めない。予定の複製でも添付はコピーしない。
Botが退出しても予定・添付は残し、再参加後に利用できる。

## 回収と障害対応

添付や予定を削除すると、DBトリガーが `attachment_deletions` にオブジェクトキーを記録する。
API・Bot・管理画面・MCP・直接SQLのどの削除でも同じ処理になる。親の予定削除をロールバックした場合、削除記録もロールバックする。

APIのワーカーが1分ごとに最大100件ずつ回収する。ファイル削除は非同期で、通常は15分の猶予後に行う。
これは発行済みPUTの再送やR2への進行中リクエストが、削除後にファイルを再作成するのを避けるため。
失敗すると次の試行時刻・試行回数をDBへ残すため、再起動しても回収できる。R2の404は削除済みとして扱う。
未確定予約は24時間で回収対象にする。一時キー `temporary/` には2日後のR2ライフサイクル削除も設定し、遅れて届いたアップロードを回収する。
ライフサイクル削除の実行時刻には遅延がある。

確認用SQL（ファイル名・URL・資格情報はログへ出さない）:

```sql
SELECT count(*) AS queued, min(next_attempt_at) AS oldest, max(attempts) AS max_attempts
FROM attachment_deletions;
SELECT count(*) AS expired_pending FROM event_attachments WHERE NOT ready AND expires_at < now();
```

回収が進まない場合は、APIが稼働しているか、4つの設定・バケット権限・ネットワークを確認する。
`attachment cleanup failed` / `attachment storage request failed` は既存のERRORログ監視に載る。
DBの削除待ち行を手で消すと回収対象を失うので、接続を修復して再試行させる。
古いAPIイメージへ戻す場合は、追加テーブル・トリガーを残し、添付の回収ワーカーが停止することに注意する。
添付メタデータを失うDB復元を行う場合は、R2とDBの時点差を確認してから運用者が孤立ファイルを照合する。

## 検証

`pnpm e2e e2e/attachments.spec.ts` は実際のAPI・PostgreSQLとローカルのR2互換モックを使う。
モックは署名付きリクエストのSigV4・サイズ・Content-Type・条件付きPUT/コピーを検査し、実資格情報は使わない。
Rustの `cargo test -p discalendar-api --test attachments` は同時予約、容量、削除トリガー、失敗回収などを確認する。

実R2ではstagingの専用バケットで、ブラウザから添付→画像表示→PDFダウンロード→削除を確認する。
加えて同じURLへの異なるサイズ・種類のPUTの拒否、許可外オリジンのCORS拒否、5分後のURL失効、回収後の404を確認する。
実R2の確認はモックテストでは代用できない。

参考: [署名付きURL](https://developers.cloudflare.com/r2/api/s3/presigned-urls/)、[CORS](https://developers.cloudflare.com/r2/buckets/cors/)、[ライフサイクル](https://developers.cloudflare.com/r2/buckets/object-lifecycles/)
