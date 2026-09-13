# MCP 接続前検証 (#216)

本体とは独立した検証専用パッケージ。Node.js 22 と pnpm 10.32.1 で実行する。

```bash
pnpm --dir scripts/mcp-poc install --frozen-lockfile --ignore-scripts
pnpm --dir scripts/mcp-poc test
```

公式プラグインをメモリ DB で起動し、metadata、CIMD 公開条件、PKCE S256、DCR 非公開、
未認証・不正トークンが MCP 処理に到達しないことを検証する。DB 接続・HTTP 待受・実トークンは不要。
メモリ DB はこの検証専用であり、公開実装では既存 PostgreSQL を使う。

この検証は Codex / Discord の認証フロー全体の PoC ではない。
Codex CLI 0.154.0 での CIMD 取得・可変 loopback port・認可画面への遷移、Discord ログイン後の復帰、
接続時同意・PKCE 交換・認証付き読み取り・解除後の拒否・Rust 側の認可を続けて検証する。
Next.js への組み込み時は、`/api/auth/[...all]` 以外の well-known URL も実際に到達するようにする。

設計と未確定事項は [MCP 設計案](../../docs/mcp-design.md) を参照。
