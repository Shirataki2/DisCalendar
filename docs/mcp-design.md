# Discord 認証付き MCP の設計案

対象: [Issue #216](https://github.com/Shirataki2/DisCalendar/issues/216)。2026-09-13 時点の検討用草案。
「合意済み」以外の提案値は合意・PoC 前であり、対応保証や公開済み仕様ではない。
#216 は設計と接続検証を扱い、公開機能は後続 Issue で実装する。

## 最初に決めること

| 判断 | 提案 | 状態 |
| --- | --- | --- |
| 初期公開 | ステージングで開発者による検証 → 招待利用 → 一般公開 | 提案 |
| 優先クライアント | Codex。最初の検証はローカル CLI 0.154.0、デスクトップは別途実機確認 | Codex 優先は合意済み |
| 操作確認 | Codex からの接続時に書き込みを同意し、以後の操作ごとの確認は要求しない | 合意済み |
| 読み取りと書き込み | まず認証付き読み取りを検証し、競合・再試行対策の後に書き込みを追加 | 提案 |
| 本番 resource | `https://discalendar.app/mcp`。ステージングは別 issuer / resource / DB | 提案 |

## 既存実装から分かったこと

- `web/src/lib/auth.ts` は Better Auth と Discord ログインを使用する。OAuth Provider のプラグインは未導入。
- `web/src/lib/login-redirect.ts` は戻り先を dashboard 配下に限定する。OAuth 認可への復帰は専用導線を設け、既存の戻り先制限を任意 URL に緩めない。
- `api/src/auth.rs` の Bearer は署名付き Web セッション値。OAuth トークンを渡すだけでは利用できない。
- `api/src/routes/member.rs` が所属確認、`api/src/routes/events.rs` が restricted・編集ロール・Discord イベント作成権限を検証する。
- `api/src/discord/mod.rs` のメンバーキャッシュは 60 秒、ギルド情報は 300 秒。ロールの割当変更だけでなく、ロール自体の権限変更も考える必要がある。
- 単一予定を取得する `find_by_id` とトランザクション内の `find_by_id_for_update` は既にある。一般利用者向け GET と version 契約は追加が必要。
- 更新はプロセス内ロックと DB の検証を併用し、Discord 操作は DB トランザクション外で先に実行する。行ロックの追加だけでは Discord の副作用まで競合から守れない。
- 削除は DB commit 後に Discord の予定をベストエフォートで削除する。MCP の結果は DB の成功と Discord の反映状況を分ける必要がある。
- `web/src/lib/auth-migrate.ts` はプラグインを含む Better Auth の DDL を起動時に適用する。検証目的のプラグイン追加でも本番 DB を変更しうる。
- `web/src/app/sw.ts` は静的資産だけをキャッシュする。現在の方針を維持し、MCP・OAuth・metadata にも `Cache-Control: no-store` を明示する。
- `web/next.config.ts` は `/local/api/*` を Rust へ転送する。「内部 API」というパス名だけでは外部からの到達を防げない。

## 認証・認可の提案

Next.js の Node.js Route Handler と公式 SDK v2 を使う。Better Auth の `mcp()`、JWT、CIMD を候補とし、
`oauthProvider()` は重ねて登録しない。独自 OAuth サーバーや追加コンテナは作らない。

ブラウザで Discord ログイン → クライアント・サーバー・操作権限の同意 → Authorization Code + PKCE S256 →
MCP 専用トークンの順に進む。Web の Cookie や Discord トークンを AI クライアントに渡さない。
未認証の metadata 発見、ログインから元の認可要求への復帰、redirect URI 照合まで PoC に含める。
接続時の同意は Codex が開く OAuth フロー内で行い、DisCalendar が許可内容を保存する。
都度確認用の画面や `confirm` 引数は追加しない。Codex 自体の実行ポリシーと OAuth の許可は別であり、
サーバーはクライアント側の確認設定を変更しない。

スコープは `guilds:read`、`events:read`、`events:create`、`events:update`、`events:delete`。
読み取りを基本とし、作成・変更・削除は個別に選ぶ。継続利用は `offline_access` の同意で区別する。
接続ごとの許可サーバー集合を保存し、新たに参加したサーバーは再同意なしに追加しない。

Rust に MCP 専用 extractor と限定ルートを設け、既存の Web セッション extractor は変更しない。
Rust も信頼済み設定の認可サーバーに認証付き introspection を行い、active・issuer・audience・期限・scope・
接続許可を検証する案とする。接続 ID とトークンの紐付け方法、プラグインの introspection が保証する項目は PoC で確認する。
リクエストで指定された introspection URL や任意の利用者ヘッダーは信用しない。

許可判定は「トークンの scope ∩ 接続の現在の scope ∩ 許可サーバー ∩ 現在の Discord / DisCalendar 権限」。
認証済み利用者と所属情報の生成までを専用経路に閉じ、予定処理・入力検証・副作用処理を既存ハンドラから必要な範囲だけ共通化する。
通常の管理・設定 API で MCP トークンが拒否されることを直接 HTTP で検証する。
DPoP が必要になった場合は入口の proof を別 URL の Rust にそのまま転用せず、内部経路の認証方式を再検討する。

接続解除後に開始した操作は拒否する。許可レコードの判定結果はキャッシュせず、判定不能時も拒否する。
実行中の操作は取り消せないため、解除画面にもこの境界を示す。アカウント削除・Discord 連携解除では接続を失効し、
Web ログアウトのみでは失効しない。再同意で既存トークンの権限が自動拡張しない設計を検証する。

トークン期限はアクセス 15 分・更新 30 日を提案する。更新トークンのローテーション・再利用検知時の失効範囲は
プラグインの実装を確認して確定する。設定画面はクライアント、許可サーバー、scope、接続日時、最終利用日時、解除を提供する。

読み取りの Discord 権限は既存キャッシュを許容し、最大 300 秒の反映遅延を案内する。
書き込みはギルド情報と本人のメンバー情報をキャッシュに頼らず再取得し、Bot のアクセスも確認する案とする。
429・タイムアウト時は古い権限で続行しない。再取得と操作の間の Discord 側変更を完全に原子的には扱えない。

## ツール契約の提案

| ツール | 必須 scope | 契約 |
| --- | --- | --- |
| `list_guilds` | `guilds:read` | 同意済みかつ現在利用可能なサーバーと実効操作権限だけを返す |
| `list_events` | `events:read` | `guild_id`、検索区間、`limit`、任意の cursor。初期値 50、最大 100 件、区間は最大 93 日 |
| `get_event` | `events:read` | `guild_id` + `event_id` で詳細と不透明な `version` を返す |
| `create_event` | `events:create` | サーバー、予定内容、`idempotency_key` を必須にし、保存内容・ID・画面 URL を返す |
| `update_event` | `events:update` | ID、`expected_version`、変更項目。省略は保持、許可した nullable 項目の null は消去 |
| `delete_event` | `events:delete` | ID、`expected_version` で単一削除。DB と Discord の結果を区別する |

一覧は区間に重なる予定を開始日時・ID の順で返す。cursor はサーバー・検索条件に束縛して検証し、
次ページの有無を明示する。ページ間の変更に対するスナップショット保証は提供せず、操作前は `get_event` で確認する。
存在しない予定と別サーバーの予定は同じ Not Found とし、他サーバーの情報を返さない。

時刻付き予定はオフセット必須の ISO 8601。内部の JST naive へ変換し、返却は `+09:00` に揃える。
終日予定は `YYYY-MM-DD` で開始日と**包含の終了日**を指定する案とする。1 日の予定は開始日 = 終了日。
検索区間は時刻の半開区間 `[start, end)` とし、終日予定は JST の開始日 00:00 から終了日の翌日 00:00 として重なりを判定する。
相対日時、オフセットなし時刻、終日と時刻付きの混在は拒否する。
Snowflake は文字列、予定 ID は既存 i32、タイトル 32 文字・説明 1000 文字・通知 10 件など既存制約を維持する。

更新では省略された説明・通知・メンション・Discord 連携を保持した完全な入力をサーバーで構成し、既存検証を通す。
`updated_at` は nullable なので、そのまま version として採用しない。
単調増加の DB revision を候補とし、Web・Bot・管理操作・Discord 対応付け変更を含む全更新経路を調べて確定する。
MCP の操作だけ revision を進める方式は不可。競合確認と DB 更新・削除は同じトランザクションで行う。
Discord を先に変更する既存フローでは、競合時の補償・結果不明まで設計することを書き込み着手の条件とする。

冪等キーは利用者・クライアント・サーバー単位、保存は最低 24 時間を提案する。
正規化した内容と結果を永続化し、同じキー・同じ内容は元の結果、内容違いは Conflict。
認証・現在の許可確認は結果の再送時にも行う。処理中は再実行せず状態を返し、結果不明は自動で新規作成しない。
Discord 作成後にプロセスが落ちた場合も含め、DB の一意制約だけで副作用の二重作成を防げるとみなさない。
期限後の再送防止と照会方法は書き込み Issue で確定し、契約未確定のまま公開しない。

成功結果には保存された予定と副作用の状態を含める。エラーは invalid input、forbidden、not found、conflict、
rate limited、outcome unknown を区別し、request ID と安全な次の操作を返す。
タイトル・説明は構造化データとして返し、ツールの指示文に埋め込まない。
tool annotations は読み取り・破壊的操作に合わせて設定するが、人の承認の証明として扱わない。

## PoC の出口と実装分割

最初は公開経路から隔離した環境とテスト DB を使う。検証クライアント名・版、SDK・Better Auth・各プラグインの正確な版、
peer dependencies、交渉された MCP 仕様版を結果とともに記録する。未検証欄を推測で埋めない。

1. **[#217 OAuth・同意・接続管理](https://github.com/Shirataki2/DisCalendar/issues/217)**: 最初に着手する。現在の接続前検証を参照し、Codex 実接続、Discord ログイン、同意、PKCE、検証用読み取り、失効と Rust への認証契約まで確認する。
2. **[#218 API 認証連携・読み取り MCP](https://github.com/Shirataki2/DisCalendar/issues/218)**: #217 に依存。専用 extractor、サーバー制限、単一取得、一覧上限、日時変換を実装する。
3. **[#219 書き込み MCP](https://github.com/Shirataki2/DisCalendar/issues/219)**: #218 に依存。全更新元に対する version、部分更新、冪等性、Discord 副作用、監査を実装する。
4. **[#220 結合検証・ガイド・公開準備](https://github.com/Shirataki2/DisCalendar/issues/220)**: #219 に依存。対象 Codex、Tunnel、停止・失効・鍵交換、公開手順を検証する。

各項目は 1 PR = 1 Issue。#216 は設計の親 Issue として開いたままにし、認証 PoC の残りを #217、Rust 側の実装検証を #218 へ引き渡す。
#217 は #216 の完了待ちにせず着手できる。実機検証の完了を推測で扱わず、証跡を親 Issue の完了条件へ反映する。

## 必須の検証と運用

- 未認証、誤 issuer / audience、期限切れ、失効、読み取りトークンでの変更、許可外サーバー、管理 API 到達を拒否する。
- 退出、ロール割当・権限変更、Bot 退出、restricted・編集ロール変更を検証する。
- 並行作成、キー再利用、並行更新・削除、Web / Bot からの変更、Discord 失敗、commit 応答喪失、途中のプロセス停止を検証する。
- OAuth と独自許可・冪等性テーブルの管理者を分ける。既存 SQLx migration は変更せず、新規 migration と必要な rollback を用意する。
- Host / Origin、必要な CORS、本文サイズ、一覧範囲、OAuth / 書き込みのレート制限を公開前に確認する。#153 と重複する責務を確認する。
- 監査は利用者・クライアント・対象 ID・操作・成否・request ID のみ。トークン・Cookie・予定本文を記録しない。
  保存期間は 30 日、閲覧は運営管理者のみを提案する。#165 の利用者向け変更履歴とは分ける。
- 全停止フラグは新規認可・トークン更新・MCP 実行を止める。既存 Web ログインと連携解除は維持する。
  再開時の既存接続の扱い、鍵ローテーション、侵害時の一括失効、Tunnel のキャッシュ除外は公開準備で手順化する。
- 実クライアントの Discord ログインとステージングの到達性は未検証。これを満たすまでは #216 の PoC 完了にはしない。

## 一次資料と確認状況

2026-09-13 に以下の公式資料を再確認した。

- [Better Auth MCP](https://better-auth.com/docs/plugins/mcp): プラグイン構成、OAuth テーブル、metadata、CIMD、DCR、入口での JWT 検証。
  `requireMcpAuth` のローカル JWT 検証だけでは独自の接続解除確認を満たさない。
- [公式 TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk)
- [MCP 認可仕様 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization)

npm レジストリで `@better-auth/mcp` / `@better-auth/cimd` 1.7.4、SDK `@modelcontextprotocol/server` 2.0.0 を確認した。
プラグインは Better Auth / core `^1.7.4` と better-call `1.4.0` を要求し、CIMD は OAuth Provider `^1.7.4` も要求する。
本体の lockfile は Better Auth 1.7.2 のため、導入時に 1.7.4 へ揃える必要がある。SDK は Node.js 20 以上を要求する。
本体の依存・DB を変更せず、固定版パッケージで接続前検証を実施した。


## Codex の接続検証条件

[公式 OpenAI MCP ドキュメント](https://developers.openai.com/codex/mcp/) とローカル CLI 0.154.0 の help を確認した。
CIMD を最初に検証し、DCR は無効で始める。metadata の CIMD 対応と public client の `none`、
loopback redirect の可変ポート・host/path 照合、認可応答の issuer を確認する。
要求された scope は同意済み scope と区別し、クライアントの要求だけで書き込みを許可しない。
Codex デスクトップの版と実接続結果は未確認で、CLI の結果をそのままデスクトップの対応保証にはしない。

### 接続前検証の結果 (2026-09-13)

再実行手順は [`scripts/mcp-poc`](../scripts/mcp-poc/README.md)。Node.js 22.22.1 で全 assertion が通過した。

- resource に対応する metadata と issuer に対応する認可サーバー metadata を取得できた。
- CIMD、public client の `none`、PKCE S256、issuer を含む認可応答のサポートが公開された。
- DCR の登録 endpoint は metadata に公開されなかった。
- 未認証・不正トークンは 401 と resource metadata の challenge を返し、MCP ハンドラへ到達しなかった。

メモリ DB とプロセス内の Request / Response による検証であり、CIMD URL の実取得、PKCE 交換、
Next.js / Tunnel の routing、実クライアント、Discord、Rust との結合はまだ検証していない。
公式 MCP プラグイン 1.7.4 のソースでは更新トークン再利用の猶予が既定 30 秒だった。
「再利用したら常に即失効」とは扱わず、猶予内外の並行更新・盗用検知を後続 PoC で確認する。
