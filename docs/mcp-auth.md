# MCP OAuth の検証環境 (#217)

設計の参照元は [#221 / ad43db9](https://github.com/Shirataki2/DisCalendar/pull/221)。
本実装は一般公開前の認証検証用。予定CRUDは #218 / #219、公開準備は #220 で扱う。
実クライアントでのDiscordログイン完了は未検証であり、#217 の完了を意味しない。

## 構成と停止

既存のBetter AuthにJWT・`mcp()`・CIMDを組み込み、`oauthProvider()`は重複登録しない。
`MCP_ENABLED` が厳密に `true` の場合だけ認可、トークン更新、MCP、metadataを有効にする。
未設定は全停止。停止中も `/mcp/connections` から既存接続を解除でき、通常のDiscordログインは継続する。
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
本番DBや本番Botを検証に使用しない。compose・本番デプロイへの有効化設定は追加していない。
`MCP_ENABLED` と `MCP_INTROSPECTION_SECRET` はサーバー実行時の変数であり、ビルド引数や `NEXT_PUBLIC_*` にはしない。

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

## 同意と失効の境界

OAuth専用 `/mcp/login` は署名付き `oauth_query` をプラグインに渡す。
プラグインが署名・期限を検証し、Discordのstate経由で認可へ復帰する。
通常の `dashboardReturnPath` の制限は維持する。

接続ごとに必ず同意画面を表示し、操作・対象サーバーを未選択から選ぶ。
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

更新はプラグインの競合制御を使用する。並行更新の片方が400になることがあり、その場合も猶予内の再送は同じローテーション済み応答を返す。
猶予外で使用済み更新トークンが再利用された場合、プラグインの失効範囲に合わせ、同じ利用者・クライアントの全接続を失効する。
別の同意で得た接続も含む。別クライアントは対象外。明示解除は指定した接続だけに限定する。
更新時の事前確認は、猶予内再送がカスタムclaim生成を通らず応答を返す経路にも適用する。

OAuth本文は16 KiB、MCP本文は64 KiB、introspectionのトークンは8 KiB、同意対象は100サーバーまで。
OAuth/MCP/同意ページにno-storeとno-referrerを付け、Cookie・トークン・署名付きqueryをアプリの監査ログへ出力しない。
検証サーバーのアクセスログにも認可URLやqueryが含まれうるため、実資格情報で運用する前にログ収集側で除外する。

## Rustへ引き渡す認証契約

検証口は `POST /mcp/introspect`。通常のRust Webセッション認証にMCPトークンを渡さない。

- Next.jsとRustだけが保持する `MCP_INTROSPECTION_SECRET`（32文字以上のランダム値）を `Authorization: Bearer ...` で送る。
- 本文は `{ "token": "<MCP access token>" }`。秘密情報をURLに含めない。
- 応答の `active: true` に `sub`（Better Auth利用者ID）、`client_id`、`connection_id`、`iss`、`aud`、`exp`、`scope`、`guild_ids` が含まれる。
- 無効なトークンは200の `{ "active": false }`。資格情報不正は401、停止中は404、DB等の判定失敗は5xx。Rustは全て拒否する。
- Rustは固定設定したこのURLだけを呼び、レスポンスをキャッシュしない。issuer/resource/期限を照合し、必要scope・対象guild・現在のDiscord/DisCalendar権限を追加検証する。
- 通信路はHTTPSまたは同一ホストの保護されたloopback。資格情報はDiscordトークンやBetter Auth secretと共有しない。

公式の `requireMcpAuth` で署名・issuer・audience・期限を確認した後、接続と利用者・クライアントの一致、Discord連携の存続、scopeの包含、失効をDBで確認する。
JWT単独検証では即時失効を保証しない。
プラグイン標準の `/api/auth/oauth2/introspect` はJWTの `sid` に紐付くWebセッションを検証するため、Webログアウト後も接続を維持する本契約には使わない。
Rustのextractor・HTTP結合実装は #218 に残る。DPoP-boundトークンは、このBearer専用内部検証口では受け付けない。

## 検証結果と再実行

2026-09-13、専用PostgreSQL 18で実行。

```bash
# public schemaを初期化するので、指定名のローカル専用DB以外はテストが拒否する
MCP_TEST_DATABASE_URL=postgresql://<user>@127.0.0.1:<port>/mcp_217_test \
  pnpm --dir web exec vitest run src/lib/mcp/oauth.integration.test.ts
```

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

未検証: 実Discordログイン→同意→Codexへのトークン返却→ツール実行の全経路、Codexデスクトップの版と結果、実クライアントが交渉したMCP仕様版、Rustプロセスからの呼び出し。
これらを確認するまで一般公開せず、#217を完了にしない。
仕様版はSDKで先に制限せず、後続の実接続で受信した `MCP-Protocol-Version` とinitialize応答を記録する。
