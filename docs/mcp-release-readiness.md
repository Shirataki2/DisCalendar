# MCP の結合検証・公開準備 (#220)

**公開判定: 未承認・未検証項目あり。一般公開・本番反映を行わない。**
この文書は #216 の残項目を追跡する。#219 のマージだけで実Codex接続の成功とはしない。
利用者向け手順は [利用ガイド](../web/src/content/docs/mcp.mdx)、実装契約は
[mcp-auth.md](mcp-auth.md) / [mcp-read.md](mcp-read.md) / [mcp-write.md](mcp-write.md)。

## 検証対象と証跡

2026-09-14時点。ステージング環境は準備済みだが、MCP対応は未実施（依頼者確認）。URL・MCP専用DB・Discordアプリの設定は未確認。
認証情報・認可URLのquery・予定本文は証跡へ含めない。

| 対象 | 版・方式 | 確認済みの範囲 | 未検証 |
| --- | --- | --- | --- |
| Codex CLI | 0.154.0、CIMD | 本作業でversionとlogin help確認。#217で登録・認可画面への302 | 実Discordログイン以降の全経路、交渉した仕様版 |
| Codexデスクトップ | 版未取得、CIMDを検証対象とする | なし | URL登録から解除まで全経路、仕様版 |
| SDK | @modelcontextprotocol/server 2.0.0 | 既存DB結合テストで2025-11-25のinitialize、tools/list、connection_info | 実Codexとの仕様交渉 |
| OAuth | Better Auth/MCP/CIMD 1.7.4、CIMD profile mcp-2026-07-28、DCR無効 | モックDiscordと専用Postgresの認可・更新・失効 | 実配信経路・実Discord |

CLIの手順は [公式MCPドキュメント](https://developers.openai.com/codex/mcp/) とインストール済みCLIのhelpを根拠とする。
CIMD profileとMCP通信仕様版は別物。成功したinitializeのprotocolVersionと後続のMCP-Protocol-Versionを記録する。
別の版・他AIクライアントへの対応保証はしない。デスクトップの版はアプリのバージョン表示から記録する。

## ステージングの受け入れ手順

本番とは別のorigin/resource/issuer/DB/Discordアプリ/Botを使う。
[mcp-auth.md](mcp-auth.md#専用dbの準備) の認証DDLとapi migrationsを適用する。
web/apiのMCP_ENABLED、共通のMCP_INTROSPECTION_SECRET、webのBETTER_AUTH_URL、apiのMCP_AUTH_ORIGIN、API_URLを確認する。
有効化変数は本番composeへ追加していない。新しいweb変数も実行時設定で、ビルド引数ではない。

次の表をCLIとデスクトップそれぞれで実施し、版・commit・日時・環境・期待値・実測値・証跡の保存先を記入する。
未実施は空欄でなく「未検証」とする。実予定を使わず、専用の検証サーバーと予定で行う。

| 手順 | 期待する結果 | 実測 |
| --- | --- | --- |
| URL登録→metadata→Discordログイン→同意 | 固定issuer/resource、CIMD、PKCE S256。クライアントIDと未選択のサーバー/操作を表示 | 未検証 |
| 読み取りだけ許可→一覧/詳細→作成を要求 | 許可内だけ取得、作成拒否 | 未検証 |
| 再接続して書き込みを許可→単一予定を作成 | ID・内容・画面URL・DB/Discordの別状態。サービス側の操作ごとの確認なし | 未検証 |
| 部分更新→Webで変更→古いversionで更新/削除 | 省略項目保持、競合拒否。最新取得後の削除成功 | 未検証 |
| 同じ作成キー/入力を再送→入力を変えて再送 | 元の結果、内容違いは拒否。重複予定なし | 未検証 |
| 15分のアクセストークン期限後に継続利用 | offline_accessで更新。更新期限/失効時は再認証案内 | 未検証 |
| 権限変更・退出・Bot退出→読み取り/書き込み | 読み取りは既存キャッシュ期限内、書き込みは現在権限で拒否 | 未検証 |
| 接続解除→操作/更新→Webログイン | 操作・更新は拒否、Webログイン継続 | 未検証 |
| Discord障害・応答喪失・結果の24時間経過 | 全面成功と案内しない。同じキーで照会、期限後は人が照合 | 未検証 |
| 新規停止→全停止→接続解除 | 下記の停止表どおり。通常Webログイン維持 | 未検証 |

障害注入は専用環境のみ。結果不明を発生させる前に管理者の照合・復旧担当を決める。
CLIのOAuth同意を取得しても、Codexのツール実行ポリシーを変更しない。

## 配信経路の確認と公開前の阻害要因

HostはBETTER_AUTH_URLのhost（port込み）と一致させる。MCP/OAuth入口は転送ヘッダーで判定を上書きしない。
Tunnelのorigin requestでHostをlocalhostへ置換していないことを実測する。
Originがある場合は同一originのみ許可。Codexの非ブラウザ通信はOrigin省略を許可し、
同意・解除フォームのOrigin省略/null/別originは拒否する。ブラウザから外部originのツール呼び出しは対象外で、CORSのワイルドカード許可は追加しない。
内部introspectionは専用Bearerと固定URLで保護し、外部公開のためにHost制限を緩めない。

| 確認 | 手順・合格条件 | 状態 |
| --- | --- | --- |
| HTTPS/Tunnel | 未認証POST /mcpの401とWWW-Authenticate、認証付き応答、403/409/429がHTMLやAccessログインへ置換されない | 未検証 |
| metadata | /.well-known/oauth-protected-resource/mcp と /.well-known/oauth-authorization-server/api/auth のresource/issuer/各endpointが外部HTTPS originを指す | 未検証 |
| callback | Discord側は /api/auth/callback/discord、Codex側はCIMDで許可された可変loopback callback。別の登録項目として照合 | 未検証 |
| Host/Origin | 不正Host、別Origin、nullを拒否。正しいHostでOriginなしのCodexが成功 | コード/自動テストと実経路を分けて記録 |
| 入力上限 | OAuth 16 KiB、MCP 64 KiB超過を拒否。chunkedでも超過を拒否し予定保存なし | 実経路未検証 |
| キャッシュ | /mcp と配下、/.well-known/、/api/auth/、/local/api/mcp/をCDN除外。no-store、Ageなし、CF-Cache-StatusがHITでないことを繰り返し確認 | 実経路未検証 |
| Service Worker | 現行は静的資産のみキャッシュ。ログイン/同意/解除後のCache Storageに上記URL・応答がないことを確認 | 実ブラウザ未検証 |
| OAuthレート制限 | Better Auth既定の制限は本番/接続元IP依存。実設定と429復帰、通常のログイン往復が妨げられないことを確認 | 未検証 |
| MCPツールの制限 | アプリにリクエスト頻度制限は未実装。書き込みロックはレート制限の代替ではない | 公開阻害 |
| エッジの制限 | #153の認証/フィード用ルールとMCP対象パスを合わせ、閾値の根拠・適用・429・復帰を検証 | 公開阻害、#153と調整 |
| ログ | 合成したquery/本文/Authorization/Cookieのマーカーがweb/api/cloudflared/Postgres/Alloy/Loki/Sentryに残らないことを検索 | 未検証、公開阻害 |

#153は認証・iCalのエッジ制限、#220はMCP/OAuthを含む受け入れとツール制限を担当する。
片方のIssueを閉じてももう片方の実測の代替にしない。制限値はトラフィックの実測から決める。
現行Alloyはweb/DBのテキストログをそのまま転送するため、no-storeやアプリ監査の本文除外だけではログ漏えい対策の完了としない。
ログ発生元で機微情報を除外し、収集設定・保存済みデータ・バックアップの範囲まで確認する。

## 停止・失効・鍵交換

設定変更後は該当する全web/apiプロセスを再起動し、実経路で確認する。停止前に実行中の操作を確認する。

| 操作 | 設定/手段 | 維持するもの |
| --- | --- | --- |
| 新規接続停止 | webのMCP_NEW_CONNECTIONS_ENABLED=false（既定は受付）。認可・ログイン開始・同意・認可コード交換を停止 | 既存アクセストークン、更新トークン、Webログイン、接続解除 |
| MCP/OAuth全停止 | web/apiのMCP_ENABLED=false | 通常Webログイン、/mcp/connectionsと解除 |
| 個別解除 | 利用者の接続管理から解除 | 別接続とWebセッション |
| 全接続失効 | 全停止して下記SQLを管理DB接続で実行 | Webのuser/account/session |

全接続失効は対象環境のバックアップとDB名を確認し、運用管理者が実施する。

```sql
BEGIN;
UPDATE mcp_connections SET revoked_at = COALESCE(revoked_at, now());
DELETE FROM "oauthRefreshToken";
DELETE FROM "oauthConsent";
COMMIT;
```

停止だけでは失効しない。侵害対応では必ず失効も行う。再開前に古いトークンの拒否と新規接続を確認する。
MCP専用introspection secretの交換は全停止→web/api双方へ同じ新しい32文字以上のランダム値を設定→全プロセス再起動→疎通→再開。
秘密値はコマンド引数・ログ・PRへ書かない。

JWT署名鍵を緊急交換する場合は全停止・全接続失効後、管理DBでjwksを削除し、web全プロセスを再起動する。
固定版JWTプラグインによる新しい鍵生成とJWKSのkid変更、古いJWTの拒否を検証してから再開する。
BETTER_AUTH_SECRETの変更はWebセッションにも影響するため、MCP専用鍵交換の代用にはしない。
通常の鍵ローテーション間隔・古い鍵の猶予運用は未検証。公開前に専用DBで上記手順を演習する。

## 障害・DB復旧・ロールバック

1. MCPを全停止し、新しい書き込みを止める。結果不明の操作を抽出し、[照合手順](mcp-write.md#結果不明からの復旧)でDiscordとDBを確定する。
2. DBバックアップと対象イメージ、適用migration一覧を確保する。全接続を失効させる。未解決状態のまま操作テーブルを削除しない。
3. [運用手順](operations.md) に従いapi/botを停止する。戻す版にないmigrationがある場合は、イメージを戻す前にDBを戻す。
4. MCP操作テーブルを戻す必要がある場合のみ api/rollback/20260913090000_mcp_event_operations.sql を実行する。
   SQLxの履歴処理は運用手順に従い、適用済みmigrationを編集しない。その後APIは全プロセスを同じ旧版にする。Webの接続解除を提供する版を維持する。
5. OAuth DDLを撤去する web/migrations/mcp/rollback.sql は検証DBの廃棄用。接続管理もテーブルを必要とするため、停止中の解除を維持する運用では実行しない。
6. DBをバックアップから復元すると接続の失効・操作記録も過去に戻る。MCPを停止したまま再度全失効し、復元時点以後のDiscord副作用を照合する。
   古いクライアントからの再送を許可しない。照合後に新規同意と新しい操作で再開する。

## 監査と #216 の残項目

書き込み監査はadmin_audit_logsに30日（毎時削除）、閲覧者は既存の管理者認証を通る管理画面利用者。
結果本文は24時間でアクセス不可、毎時消去により最大約25時間保持。再実行防止の墓標は期限なし。
バックアップの保存期間は [バックアップ運用](../infra/README.md) と実設定を確認する。実際の管理者一覧・Loki閲覧者・保持設定は未確認。
墓標の保持方針、バックアップからの消去期限も公開前に運用責任者が確認する。

| #216 完了条件 | 根拠と行き先 |
| --- | --- |
| 公開範囲・版・仕様 | この文書の検証対象。実Codex交渉とデスクトップ版は #220 に残す |
| URL→Discord→同意→PKCE→読み取りPoC | #217のモックDB結合テスト。実接続は #220 に残す |
| Rustまでの認証・scope・guild・管理API拒否 | #218、api/src/mcp.rsのテスト、web/e2e/mcp-origin.spec.ts。実経路は #220 |
| 同意・解除・期限・権限反映・確認方針 | mcp-auth/read/write.md。新規停止は本変更、実機は #220 |
| 入出力・日時・競合・再送・副作用 | #218/#219、mcp-read/write.mdと各テスト。実Discordは #220 |
| 異常系 | OAuth結合テストとRust MCPテスト。利用者への実案内は上記受け入れ表 |
| DB・rollback・レート制限・監査・公開経路 | 実装docsと本書。頻度制限/配信/ログ/復旧演習は未完了で #220、エッジ共通ルールは #153 |
| 設計記録・Issue分割 | #217→#218→#219→#220。設計原稿は #221/ad43db9（未マージのためローカルに存在しない） |

未検証のチェックを付けず、#216/#220は実測と公開阻害要因の解消まで完了扱いにしない。

## この変更のローカル検証

- `pnpm --dir web lint`: 成功（既存のBiome schema版差異のinfoのみ）。
- `pnpm --dir web exec next typegen` 後の `tsc --noEmit`: 成功。
- 通常Vitest: 157件成功。専用DBが必要なテストは別実行。
- PostgreSQL 18.6の一時DB `mcp_217_test` を用いたOAuth結合テスト: 14件成功。
  新規停止中の読み取り・更新・解除・Webセッション維持、認可とコード交換の停止、公開入口のHost/Origin拒否を含む。
  Discordはモックであり、ステージング成功の証跡にはしない。
- `pnpm --dir web build`: 成功。最終実行は専用テスト用の認証・DB設定を指定。
- Playwrightで利用ガイド表示、390px幅の横はみ出しなし、MCP/OAuth停止時の404/no-store、ログインページのReferrer-Policyを確認。
