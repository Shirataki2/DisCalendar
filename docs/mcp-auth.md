# MCP OAuth・接続管理 (#217)

設計の参照元は [#221 / ad43db9](https://github.com/Shirataki2/DisCalendar/pull/221)。
予定の読み取りは [mcp-read.md](mcp-read.md)、書き込みは [mcp-write.md](mcp-write.md)、公開準備は [#220 の検証・運用手順](mcp-release-readiness.md) で扱う。
認証実装の #217 は完了済み。実機の最新の確認結果と未検証項目は [公開準備の検証結果](mcp-release-readiness.md) にまとめる。

## 構成と停止

既存のBetter AuthにJWT・`mcp()`・CIMDを組み込み、`oauthProvider()`は重複登録しない。
`MCP_ENABLED` が厳密に `true` の場合だけ認可、トークン更新、MCP、metadataを有効にする。
未設定は全停止。停止中も `/mcp/connections` から既存接続を解除でき、通常のDiscordログインは継続する。
新規接続のみ停止する場合は `MCP_NEW_CONNECTIONS_ENABLED=false` にする。既存接続の利用・更新と解除は継続する。
設定変更後はNext.jsを再起動する。再開すると未解除の接続は有効に戻るため、侵害対応では停止だけでなく解除する。

| 項目 | 設定 |
| --- | --- |
| Node.js | 22.22.1（検証時） |
| Better Auth / MCP / CIMD / OAuth Provider | 1.7.4（固定） |
| 公式 MCP SDK | `@modelcontextprotocol/server` 2.0.0（固定） |
| Zod | lockfileの4.4.3 |
| issuer | `${BETTER_AUTH_URL}/api/auth` |
| resource | `${BETTER_AUTH_URL}/mcp` |
| 登録方式 | CIMD。DCRは無効 |
| アクセストークン | JWT、15分 |
| 更新トークン | 発行・更新時から30日、ローテーション、再利用猶予30秒 |

`BETTER_AUTH_URL` はパス・query・fragmentを含まないoriginにする。
HTTPSを使う。ローカル開発のみloopback HTTPを許容する。
本番DBや本番Botを検証に使用しない。composeは既定でMCP無効とし、明示的な有効化手順は下記に記載する。
`MCP_ENABLED` と `MCP_INTROSPECTION_SECRET` はサーバー実行時の変数であり、ビルド引数や `NEXT_PUBLIC_*` にはしない。

## 既存認証DBとの互換性

Better Auth 1.7.3以降は `account.issuer` を書かなくなったが、既存1.7.2のDBにはNOT NULL列と `(issuer, accountId)` の一意制約がある。
[公式アップグレードガイド](https://better-auth.com/docs/guides/1-7-upgrade-guide)の制約緩和とは別に、このアプリでは既存列・既存データ・一意制約を維持する。
共有の `auth-schema.mjs` でissuerを宣言し、Discord専用の既定値 `local:oauth:discord` をアダプターから書く。
これによりDBの列削除・型変換や本番の事前DDLを必要とせず、旧版への切り戻しも妨げない。
起動時migration・手動migration・E2EのDB準備で同じ定義を使用する。Discord以外の認証プロバイダーを追加するときは、この既定値も見直すこと。
結合テストではDB側のDEFAULTを外し、1.7.2と同じNOT NULL制約下でアカウント作成できることを確認する。

## 専用DBの準備

既存の開発・本番DBとは異なる検証用DBとDiscord OAuthアプリを用意し、検証用の `DATABASE_URL` を指定する。
通常の `pnpm db:migrate` で基本認証テーブルを作ってから、以下を**一度だけ**適用する。
`AUTO_MIGRATE` の起動時処理は基本認証テーブルのみを扱い、MCPのDDLを適用しない。

```bash
# web/ で、DATABASE_URL が専用DBを指していることを確認して実行
psql "$DATABASE_URL" -v ON_ERROR_STOP=1 --single-transaction -f migrations/mcp/001_oauth.sql -f migrations/mcp/002_connections.sql
```

`001_oauth.sql` は固定版の `getMigrations(auth.options).compileMigrations()` によるDDL。
`002_connections.sql` は接続単位のサーバー・scope・作成日時・最終利用日時・失効日時を管理する。
今後の変更は新規migrationで行い、適用済みSQLを書き換えない。apiのSQLx migrationsとは別管理で、本番のapi起動時には適用されない。

戻す場合はMCPを停止し、検証DBに `migrations/mcp/rollback.sql` を適用する。
OAuthの鍵・クライアント・トークン・接続は削除され、再導入後は再同意が必要。
基本認証のuser/account/session/verificationは残る。

## compose環境での本番有効化

本番では既存の認証DBにMCP専用テーブルを追加する。検証用DBやDiscordアプリを本番へ流用しない。
composeでのMCP利用は、apiコンテナからも到達できる公開HTTPSの `BETTER_AUTH_URL` が前提。apiはこのoriginの `/mcp/introspect` に問い合わせる。コンテナ内のlocalhostはwebを指さないため、localhost URLによるcomposeのMCP利用は対象外とし、ローカル検証は [開発環境](development.md) のホスト上で行う。
依頼者が本番有効化を指示した場合に実施し、#228 / #229 の残検証を完了扱いにはしない。

APIはMCP認証テーブルをSQLコンソールの保護対象とする。テーブル適用後は、この保護を含むv3.13.0以降のAPIを配布し、旧APIの再起動でSELECT権限を与えない。

1. 対象のcomposeプロジェクト名・DB・配布する版を確認し、[運用手順](operations.md#db-のバックアップと復元)に従ってDBをバックアップする。
2. `MCP_ENABLED=false` のまま、対象リリースの `web/migrations/mcp/001_oauth.sql` と `002_connections.sql` を一度だけ適用する。基本認証テーブルが既にある本番DBに、同じトランザクションで両方を適用する。適用済みの場合は再実行しない。一部だけ存在する場合は止めて状態を調査する。

   ```bash
   # 対象ホストの本番composeディレクトリで実行。SQLは対象リリースのものを配置しておく。
   mcp_ddl=$(cat web/migrations/mcp/001_oauth.sql web/migrations/mcp/002_connections.sql) &&
   printf '%s\n' "$mcp_ddl" | \
     docker compose exec -T db psql -U discalendar -d discalendar -v ON_ERROR_STOP=1 --single-transaction
   ```

3. ホストの `.env` に `MCP_INTROSPECTION_SECRET`（`openssl rand -hex 32` で生成、画面・ログへ出さない）と `MCP_ENABLED=true` を設定する。共有秘密は環境ごとに分ける。`BETTER_AUTH_URL=https://discalendar.app` がapiの `MCP_AUTH_ORIGIN` にも渡る。
4. 対象版を Deploy production で配布する。web/apiが再作成され、設定が反映される。設定だけ変更するときも `docker compose up -d --no-deps api web` を使う。
5. 公開metadataのissuer/resourceが本番originであること、未認証MCPが401で拒否されること、通常ページとDiscordログインを確認する。実Codexで同意・予定の読み取り・接続解除を確認する。

全停止は `.env` の `MCP_ENABLED=false` をweb/apiへ再適用する。停止だけでは既存接続は失効しない。
DBを戻す場合は先にweb/apiのMCPを停止し、全接続を失効してから対象版のrollbackを使う。バックアップやSQL実行出力に認証情報を含めて公開しない。

## 同意と失効の境界

OAuth専用 `/mcp/login` は署名付き `oauth_query` をプラグインに渡す。
プラグインが署名・期限を検証し、Discordのstate経由で認可へ復帰する。
通常の `dashboardReturnPath` の制限は維持する。

接続ごとに必ず同意画面を表示し、要求された選択可能な操作（継続利用を含む）と、その時点で本人とBotが参加するサーバーを初期状態で全選択する。個別変更・全選択・全解除ができ、明示的に「選択した内容を許可」を押すまで接続は保存しない。後から参加したサーバーは自動追加しない。
クライアントIDはそのまま表示し、未検証のブランド名やロゴに置き換えない。
同意のPOSTで現在の所属とBot参加を再取得し、要求scopeの部分集合とサーバーIDの包含を検証する。
この確認に失敗した場合は許可を保存しない。取得処理は既存Discord/APIキャッシュを利用するため、退出の反映には既存のキャッシュ期限がある。
後続の予定操作はRustで現在の権限を再検証すること。

同意ごとに新しい `connection_id` を作り、プラグインの `referenceId` を通して認可コードと更新トークンに保存する。
JWTには `connection_id` を追加する。再同意で古いレコードのscopeやサーバー集合を書き換えない。
直接の `/api/auth/oauth2/consent` / `continue` は拒否する。
接続解除は許可レコードを先に失効し、その接続の更新トークンとプラグイン同意を削除する。
判定結果はキャッシュせず、解除後に開始した操作は拒否する。実行中の操作は取り消せない。

Discordのaccount行またはuser行の削除は外部キーで許可と更新トークンを削除する。
同じDiscordアカウントを再連携しても以前の許可は復活しない。
Webログアウトだけでは許可を削除しない。
アカウントメニューの接続管理へのリンクはMCP停止中も表示し、既存接続を解除できる。

更新はプラグインの競合制御を使用する。並行更新の片方が400になることがあり、その場合も猶予内の再送は同じローテーション済み応答を返す。
猶予外で使用済み更新トークンが再利用された場合、プラグインの失効範囲に合わせ、同じ利用者・クライアントの全接続を失効する。
失効対象はDBの利用者・クライアントから決め、本文のclient_idの省略・不一致でも失効させる。
別の同意で得た接続も含む。別クライアントは対象外。明示解除は指定した接続だけに限定する。
更新時の事前確認は、猶予内再送がカスタムclaim生成を通らず応答を返す経路にも適用する。

OAuth本文は16 KiB、MCP本文は64 KiB、introspectionのトークンは8 KiB、同意対象は100サーバーまで。
OAuth/MCP/同意ページにno-storeとstrict-originを付け、Cookie・トークン・署名付きqueryをアプリの監査ログへ出力しない。
strict-originはRefererをオリジンだけに限定し、フォームPOSTのOriginを維持する。no-referrerではOriginがnullになり、同意・接続解除の同一オリジン検証に失敗するため使用しない。Originがnull・欠落・別オリジンの要求は引き続き拒否する。
検証サーバーのアクセスログにも認可URLやqueryが含まれうるため、実資格情報で運用する前にログ収集側で除外する。

## Rustへ引き渡す認証契約

検証口は `POST /mcp/introspect`。通常のRust Webセッション認証にMCPトークンを渡さない。

- Next.jsとRustだけが保持する `MCP_INTROSPECTION_SECRET`（32文字以上のランダム値）を `Authorization: Bearer ...` で送る。
- 本文は `{ "token": "<MCP access token>" }`。秘密情報をURLに含めない。
- 応答の `active: true` に `sub`（Better Auth利用者ID）、`client_id`、`connection_id`、`iss`、`aud`、`exp`、`scope`、`guild_ids` が含まれる。
- 無効なトークンは200の `{ "active": false }`。資格情報不正は401、停止中は404、DB等の判定失敗は5xx。Rustは全て拒否する。
- Rustは固定設定したこのURLだけを呼び、レスポンスをキャッシュしない。issuer/resource/期限を照合し、必要scope・対象guild・現在のDiscord/DisCalendar権限を追加検証する。
- 通信路はHTTPSまたは同一ホストの保護されたloopback。資格情報はDiscordトークンやBetter Auth secretと共有しない。

公式の `requireMcpAuth` で署名・issuer・audience・期限を確認した後、接続と利用者・クライアントの一致、Discord連携の存続、失効をDBで確認し、scopeはトークンと接続の現在値の共通部分に限定する。
JWT単独検証では即時失効を保証しない。
プラグイン標準の `/api/auth/oauth2/introspect` はJWTの `sid` に紐付くWebセッションを検証するため、Webログアウト後も接続を維持する本契約には使わない。
Rustのextractor・HTTP結合実装と検証方法は [mcp-read.md](mcp-read.md) を参照。DPoP-boundトークンは、このBearer専用内部検証口では受け付けない。

## 検証結果と再実行

2026-09-13、専用PostgreSQL 18で実行。

```bash
# public schemaを初期化するので、指定名のローカル専用DB以外はテストが拒否する
MCP_TEST_DATABASE_URL=postgresql://<user>@127.0.0.1:<port>/mcp_217_test \
  pnpm --dir web exec vitest run src/lib/mcp/oauth.integration.test.ts
```

同意画面・接続管理のPC/スマートフォンとキーボード操作は `MCP_ENABLED=true pnpm --dir web e2e e2e/mcp-ui.spec.ts` で確認する。E2E専用DBにMCPのDDLも適用し、Discordはモックを使用する。

通常の `pnpm test` はDB結合テストをskipし、選択値の検証を含むユニットテストを実行する。CIのwebジョブは専用PostgreSQLサービスを使い、結合テストも実行する。
結合テストはCIMD取得・Discord所属取得だけをモックし、PostgreSQL、プラグイン、署名、認可コード交換、更新、失効を実際に動かす。
Discordのコード交換とプロフィール取得をモックしたログイン復帰も検証するが、実Discordログイン成功とは扱わない。

確認済み（Webテスト計156件、うちDB結合9件が通過）:

- CIMD metadata、DCR非公開、PKCE S256、可変loopback port、stateとissuerの返却
- 初回/再同意、認可コード交換と再利用拒否、誤PKCEの拒否、認証付き接続情報の読み取り
- scope/サーバー/署名の改ざん、未同意、不正JWT、期限切れ、誤issuer/audienceの拒否
- 再同意時の接続分離、解除後の実行・更新拒否、並行更新、猶予内再送と猶予外失効
- Webログアウトで維持、Discord連携・アカウント削除で失効、introspection資格情報、全停止、rollback後の基本認証テーブル維持
- 公式SDKで2025-11-25のinitialize、tools/list、connection_infoを認証付き実行（実Codexの交渉結果とは区別）
- 実Next.js経由でwell-knownの200とno-storeを確認
- 実Codex CLI 0.154.0のCIMD接続: client_id `https://chatgpt.com/oauth/codex/client.json`、loopback `127.0.0.1:<可変port>/callback`、PKCE S256、resourceを確認。認可要求をNext.jsへ送信し `/mcp/login` への302を確認

Codex CLIの接続例（[公式MCPドキュメント](https://developers.openai.com/codex/mcp/)と0.154.0のhelpを確認）:

```bash
codex mcp add discalendar-test --url http://127.0.0.1:35217/mcp
codex mcp login discalendar-test --oauth-client-registration cimd \
  --scopes guilds:read,events:read,events:create,events:update,events:delete,offline_access
```

2026-09-14 22:00 JST、ステージングの実機確認で15分経過後の更新トークンによる継続利用、接続解除後の操作拒否・Webログイン維持を確認済み（依頼者報告）。
Codexデスクトップ26.908.40834でも確認済み。対象commitと8項目の結果、交渉仕様版の値などの引き継ぎ項目は [公開準備の検証結果](mcp-release-readiness.md) を参照。
#216 / #220 は受入れ済み。未完了項目は #228 / #229、一般公開・本番反映は別途の指示で扱う。
仕様版はSDKで先に制限せず、後続の実接続で受信した `MCP-Protocol-Version` とinitialize応答を記録する。
