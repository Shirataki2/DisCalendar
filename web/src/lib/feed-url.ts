// iCal フィード (#95) の URL。
// 外部カレンダーは api を直接叩けない (api のポートは公開していない) ので、URL は web のオリジンで出し、
// next.config.ts の rewrites が api の `GET /feeds/{token}.ics` へ中継する

/** フィードのパス (rewrites の source と api のルートに対応) */
export function feedPath(token: string): string {
  return `/feeds/${token}.ics`;
}

/**
 * 外部カレンダーに登録する URL。`origin` はブラウザの `window.location.origin`
 * (ローカル / staging / 本番のどれで開いていても、その環境の URL になる)
 */
export function buildFeedUrl(origin: string, token: string): string {
  return `${origin.replace(/\/+$/, "")}${feedPath(token)}`;
}
