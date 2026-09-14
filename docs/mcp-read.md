# MCP の予定読み取り (#218)

検証環境限定。OAuth・同意・接続解除は [mcp-auth.md](mcp-auth.md) を参照。
書き込みは [mcp-write.md](mcp-write.md)。
実Codex・Discordでの最新の確認結果と未検証項目は [公開準備の検証結果](mcp-release-readiness.md) を参照。
公開準備は #220、一般公開・本番反映は別途の指示に従う。

## 起動と認証境界

専用DBでOAuthの準備を済ませ、webとapiの両方で `MCP_ENABLED=true` にする。
apiの `MCP_AUTH_ORIGIN` はwebの `BETTER_AUTH_URL` と同じorigin、
`MCP_INTROSPECTION_SECRET` はwebと共通の32文字以上の専用ランダム値にする。
既定は停止。設定不足・不正なoriginはapi起動時に拒否する。
webは既存の `API_URL` に接続する。検証時は同一ホストのloopbackに向け、
別ホスト間ではHTTPSとアクセス制限で通信路を保護する。本番composeへの有効化設定は追加していない。

Next.jsの `/mcp` はBearerだけをRustへ渡し、Cookieや利用者IDは渡さない。
Rustの `McpUser` は毎回固定URLの `/mcp/introspect` を専用資格情報で呼び、
active・issuer・resource・期限・接続ID・client IDを確認する。
検証済み `connection_id`・`sub`・`client_id` と共有DBの接続を照合し、
同意時に `discord_account_id` へ固定したDiscord accountだけから本人を解決する。
同じ利用者の別アカウントへ代替せず、入力の利用者IDは受け付けない。
判定失敗、停止、失効、通信障害は拒否する。introspectionの通信失敗種別・HTTPエラーステータスはERRORに記録する。リダイレクトを追わず、資格情報・応答本文はログに出さない。

実効scopeはトークンと接続の現在のscopeの共通部分。同意時のサーバー集合は拡張せず、
操作ごとに既存 `DiscordClient::member_access` で本人の所属とBotの参加を確認する。
読み取りはWeb同様、restrictedモードでもメンバーに許可する。
メンバーのキャッシュは60秒、ギルドは300秒なので、退出や権限変更の反映にはその遅延がある。
接続失効はキャッシュせず、解除後に開始した次の操作から拒否する。処理中の操作は取り消さない。

通常の `AuthUser`・管理/設定/編集APIは変更していない。
`/local/api/:path*` の公開rewriteから内部の `/mcp/*` を直接呼んでも同じextractorを通り、
通常APIへの権限拡張や本人の差し替えはできない。

## ツールとRust HTTP契約

すべて読み取り専用。Rust側はPOST・JSON、応答はno-store。
MCPは同じJSONを `structuredContent` とtext contentに返す。
サーバー名・タイトル・説明などは未信頼のデータであり、ツールの説明文や指示として埋め込まない。

| ツール | Rustルート | 入力 | 応答 | scope |
| --- | --- | --- | --- | --- |
| list_guilds | `/mcp/guilds` | `{}` | `{guilds: [{guild_id, name}]}` | guilds:read |
| list_events | `/mcp/events/list` | `{guild_id, start, end, limit?, cursor?}` | `{events: [...], next_cursor: string \| null}` | events:read |
| get_event | `/mcp/events/get` | `{guild_id, event_id}` | `{event: {...}}` | events:read |

`list_guilds` は同意済みサーバーのうち現在利用できるものだけを返す（最大同意100件）。
所属確認は最大4並列で実行し、同意した順序を維持する。
Discord障害で所属を判定できなければ一覧全体が失敗し、部分的な一覧を成功扱いにしない。
予定は常に `guild_id + event_id` で絞る。許可外・非メンバー・Bot未参加は403、
許可されたサーバー内に該当予定がなければ404。Snowflakeは常に文字列、予定IDは正の32bit整数。

予定のフィールドは通常APIの `Event` と同じで、以下を変更・追加する。

- 時刻指定の `start_at` / `end_at` はオフセット付きISO 8601（JSTの `+09:00`）。
- 終日の `start_at` / `end_at` は `YYYY-MM-DD`。終了日を含む。DBの時刻部分は無視する。
- `created_at` / nullableな `updated_at` もオフセット付きISO 8601。
- `version` は下記の不透明文字列。通知設定や作成者など他のフィールドは通常APIと同じ。

## 日付とページ送り

検索の `start` / `end` はオフセット必須のISO 8601。日付だけやオフセットなしは拒否する。
JST naiveへ変換後、正の期間かつ最大93日とする。検索は `[start, end)` に重なる予定を返す。
終日は開始日00:00から包含終了日の翌日00:00までとし、終了日昼の検索でも落とさない。
時刻指定で終了と検索開始が一致する予定は含めず、長さ0の予定は開始の一点として判定する。

初期50件、1〜100件。開始日時・IDの昇順で取得し、続きがあれば `next_cursor` を返す。
cursorはHMAC署名で、本人・接続・サーバー・UTCオフセットを正規化した検索期間・件数・最後の開始日時とIDに束縛する。
条件変更、別接続への流用、改ざんは400。次ページでも認証・scope・所属を再確認する。
検索結果はスナップショットではないため、ページ間に予定の日時を変更すると重複・欠落がありうる。
一覧を確定して扱う必要がある場合は最初から取得し直す。

## version と後続の更新契約

`sha256-v1:<base64url>` は `EventRow` の固定フィールド順でserializeした生の保存値全体
（通知JSON、作成/更新日時・作成者、Discord連携IDを含む）のSHA-256。
nullableなupdated_at単独には依存せず、NULLのまま予定内容が変わっても変化する。
連番や日時ではなく現在の保存内容の一致を示し、完全に同じ保存内容に戻った場合は同じ値になる。

#219の更新・削除は、トランザクション内で予定行をロックし、関連する連携情報も確定させてから
この同じ計算を使って `expected_version` と照合し、不一致は409とする。
読み取り時のversionが永久に更新権限を与えることはなく、書き込み時にも失効・scope・編集権限を確認する。
単調増加のrevisionが必要になった場合は新規migrationと全writer（api/bot）の対応を別途行う。

## 検証

- Rustの `mcp::tests` は専用PostgreSQLとintrospection/DiscordのHTTPモックを使い、
  正常読み取り・空結果・初期50/最大100件・cursor境界と改ざん・終日/オフセット・version・本人とguildの境界・
  scope不足/期限切れ/失効/障害・通常管理APIの拒否を確認する。
- Webの `read-tools.test.ts` は公式SDKで3ツールを呼び、入力検証、構造化出力、Bearerのみの転送を確認する。
- OAuth結合テストは実署名とDBで現在scopeの縮小を確認する。
- `e2e/mcp-origin.spec.ts` は実Next.jsの公開rewriteを経由した通常APIと停止中の内部ルートの拒否を確認する。
- 新しいSQLはパラメーター付きruntime queryで、DB結合テストが検証する。`query!` の変更・DB migrationはなく、SQLxメタデータの追加は不要。

実Codex/Discord認証から予定取得までの全経路、実Codexの仕様交渉は未確認。
認証サービスとRustのHTTP境界は上記テストで検証するが、実接続成功とは区別する。
