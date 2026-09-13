# MCP の予定書き込み (#219)

検証環境限定。認証設定は [mcp-auth.md](mcp-auth.md)、日時・読み取りの契約は
[mcp-read.md](mcp-read.md)。一般公開は #220。実Codex・Discordの接続確認は別途必要。

## 入力と権限

書き込みは接続時に同意する。操作ごとの確認画面や `confirm` は設けない。

| ツール | RustのPOSTルート | 必須scope | 入力 |
| --- | --- | --- | --- |
| create_event | `/mcp/events/create` | events:create | guild_id, idempotency_key, changes |
| update_event | `/mcp/events/update` | events:update | 上記 + event_id, expected_version |
| delete_event | `/mcp/events/delete` | events:delete | guild_id, idempotency_key, event_id, expected_version |
| get_event_operation | `/mcp/events/operation` | 元の操作のscope | guild_id, idempotency_key |

通常APIはMCPトークンを受け付けない。各操作・再送・結果照会で接続の有効性・現在scope・同意済みguildを確認し、
Discordからギルド、本人のメンバー情報、Botのメンバー情報を取り直す。
読み取りや「権限を再取得」ボタンのキャッシュ・間隔制限は使わず、取得不能・429時に古い値へ戻らない。
restrictedモードは管理権限または編集ロールを要求する。Discord連携を新しく付けるときは本人とBotの
Create Events権限、既存の連携更新ではBotの同権限を要求する。結果の再送も元の操作で要求した権限を再確認する。
処理開始後の認可取消は実行中の操作を取り消さず、次の操作から拒否する。

`changes` は通常の予定入力のフィールドを使う。作成には `name`, `color`, `start_at`, `end_at` が必要。
更新の省略項目は保持し、完成した入力を既存の上限・メンション対象検証へ渡す。
`description: null` は説明を消去、`notifications: null` / `notification_mentions: null` は空配列、
`discord_scheduled_event: null` は連携解除。他の必須値のnullや未知のフィールドは拒否する。
時刻指定はオフセット必須ISO 8601からJST naiveへ変換する。終日は `YYYY-MM-DD` と包含終了日を使う。
MCPで `is_all_day` を明示するときは、同じ値を指定する場合もその形式で開始・終了の両方を指定する。
フラグ省略時の部分更新は保存済みの終日設定に従いAPIで検証する。

## 保存結果と再送

結果は `request_id`, `event_id`, 保存内容の `event`（version込み）, カレンダーの `url`、判明している反映対象の `discord_event_id` と次の状態を返す。
削除時の `event` は削除直前の保存内容であり、現存する予定ではない。

| フィールド | 値 | 意味 |
| --- | --- | --- |
| database | succeeded | 予定と操作記録のcommitを確認した |
| discord | not_required | Discord操作なし |
| discord | succeeded | Discord応答と対応付けの保存を確認した |
| discord | failed | 4xxなどで反映の拒否を確認した。予定のDB保存は成功している |
| discord | unknown | 接続喪失・5xx・不正応答・途中停止・対応付け保存失敗等。全面成功とは案内しない |

HTTP応答が届かない、DBのcommit応答が失われる、5xxになる場合は全体の成否を推測せず、同じキーで照会する。
照会の404は「その時点で記録が見つからない」であり、処理中の可能性がある。少し待って同じ内容・キーを再送する。
同じギルドに別の書き込みが進行中なら409を返す。新しいキーに変えず、完了後に再送する。

- キーは空白を除く印字可能ASCIIの1〜128文字。同じ利用者（OAuth sub）・client_id・guild_id内で一意。
  接続を作り直しても同じ利用者・クライアントならキーは共有する。作成だけでなく更新・削除にも要求する。
- 同じキー・同じ入力の再送は元の結果を返す。比較は操作種別・対象ID・expected_version・changesのJSONを
  キー順で正規化したSHA-256。値や省略の違いは別の入力なので、再送時は入力をそのまま保持する。
- 同じキーで別内容は409。期限は最初のDB保存から24時間。期限後も409で、キーを新しい操作へ使い回さない。
- 結果本文は24時間でアクセス不可となり、毎時の処理で消去する（稼働中の実保持は最大約25時間）。
  user/client/guild、キーと入力のハッシュ、操作種別、event_id、判明しているDiscord ID、時刻、外部状態・必要権限は再実行防止の墓標として保持する。
- 結果照会は現在の予定を取得する機能ではない。後でWebから変更しても、再送は当時の結果を返す。
  現在の内容は `get_event` で確認する。

## 競合とDiscord反映

`expected_version` は #218 の保存内容ハッシュをそのまま使う。トランザクション内で予定を `FOR UPDATE` し、
対応付け込みのハッシュと照合してから変更する。不一致は409、削除済みは404。
Web・管理画面の更新は更新日時も保存し、Botの作成は新しいIDとなる。対応付けの追加・変更・削除もハッシュを変える。
nullableなupdated_at単独には依存しない。保存内容を完全に元に戻した場合は同じversionになる契約は変更しない。

調査したwriterは `api/src/routes/events.rs`、管理の単一予定と一括削除、`bot/src/models/events.rs`（作成のみ）、
`api/src/models/event_links.rs`。Botには既存予定の更新・削除・対応付け変更経路はない。
管理SQLコンソールは読み取り専用で、手動DB操作は通常運用のwriterに含めない。

MCPは**DBと操作記録を先に同一トランザクションで保存し、その後Discordへ一度だけ送る**。
外部操作が必要な場合は送信前にunknownを永続化する。送信前後に停止しても再送から外部操作を自動実行しない。
Discordの失敗を理由に確定済みDBを巻き戻す補償は行わない。結果で部分成功を伝え、下記の復旧へ進む。
Webhookは通常APIと同じoutboxへDB変更と同時に積み、配信完了はこのDiscord状態に含めない。
POST応答で判明したDiscord IDはリンク保存より先に操作記録へ確定する。リンク保存失敗時も照合用IDを保持する。
連携IDを新規保存・差し替えした場合は、確定したIDを含む `event.updated` を同じトランザクションで追加する。
Discord更新が404なら本人のCreate Events権限を確認し、一度だけ再作成して対応付けを差し替える。
再作成も事前に保存したunknownで保護し、応答喪失・途中停止後の再送では繰り返さない。

既存WebのDiscord先行経路・補償も含め、APIの作成・更新・削除・管理操作を共通のPostgreSQLセッション勧告ロックで
ギルド単位に直列化する。ロックは通常プールとは別の接続で保持し、競合は即409、キャンセル時も接続終了で解放する。
専用接続はプロセス全体で最大5本（通常の `DATABASE_MAX_CONNECTIONS` とは別枠）。
上限時は接続を作らず即409を返す。通常プールが1本でも処理できる。
全APIプロセスを同じ版へ更新すること。旧APIとの混在中はこの排他を保証しない。
Botの作成は別IDなので既存予定への反映順には干渉しない。

### 結果不明からの復旧

unknown、またはDiscord削除の確定失敗を持つ予定は、新しいMCP操作やWebの編集・削除を409で止める。結果本文が失効してもこの停止は維持する。
管理者がDiscord側とDBを突き合わせるまで、自動的な再作成は行わない。
管理画面のギルド全予定削除も、ギルド内に未解決操作があれば409で停止し、照合に必要な予定を残す。

1. 管理者がDB接続を使い、`mcp_event_operations` のuser/client/guild/キーのハッシュを指定して操作を特定する。
   通常の管理SQLコンソールにはこのテーブルの閲覧権限を与えない。
2. Discord側の予定を確認し、存在するなら `event_discord_links` を正しいIDへ合わせる。不要な孤児があればDiscord側で整理する。
   応答喪失ではID自体が不明な場合もあるため、時刻・サーバー・保存内容を突き合わせ、人が確定できるまで停止を維持する。
3. 同じギルドの `event_writer` 勧告ロックを取得したDBセッションで、対象の `discord_state` を確定した状態へ更新する（削除の後始末が完了した場合は `succeeded`）。
   結果が未失効なら `result.discord` と `result.event` も現在の対応付けに揃える。複数の未解決の操作があれば全件確認する。
4. 操作者へ `get_event` で内容・versionを取り直すよう案内する。別の予定を新しいキーで作ることを復旧手段にしない。

作成・更新の確定拒否（failed）は、権限や入力を直し、予定を再取得してから新しい更新として反映できる。
連携解除はDBの対応付けを先に外すため、Discord削除に失敗した場合は記録したDiscord IDで管理者が整理してから停止を解除する。
DB保存が成功した作成を新しいcreateとして繰り返さない。

## 監査・DB・検証

`admin_audit_logs` に `mcp.event.create/update/delete` を記録する。
操作者の内部ID・Discord ID、クライアント・接続ID、guild/event ID、操作種別、DB/Discord状態、request ID、時刻のみで、
本文・通知・トークン・生の冪等キーは入れない。DB確定時と外部結果確定時を記録し、保存期間は30日（毎時削除）。
既存の管理者認証を通る監査画面から閲覧できる。再送の保存結果は監査ログと分離する。
バックアップには通常のDBバックアップ保存期間が適用される。

新規migrationで `mcp_event_operations` を追加する。既存eventsの保存形式やbotのクエリは変更しない。
追加SQLはパラメーター付きruntime queryで結合テストを行い、既存query!の変更がないためSQLxメタデータは変更不要。
戻す場合はMCPを停止し、旧APIへ戻した後 `api/rollback/20260913090000_mcp_event_operations.sql` を実行する。
墓標が消えるため、再有効化する際は既存接続を失効させ、クライアントに古い操作を再送させない。

テストは省略/null・上限/日時、scopeとguildの拒否、キャッシュ後の退出・429、restricted/編集ロール、
別writerによるversion変更、同時作成・並行更新、同じキーの再送・別内容・期限後、Discord成功/拒否/不正応答と途中停止、
PostgreSQLがcommit後に応答だけ切断された場合の再送とrollbackを検証する。
実Codexからの認証・実Discordへの反映はローカルHTTPモックとは別の受け入れ確認として残る。
