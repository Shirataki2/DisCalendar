// 支援ページ (/donation) のボタンの飛び先。どれも Stripe 側で発行した URL を実行時の環境変数で受け取り、
// app/donation/<種類>/route.ts がリダイレクトする (静的な支援ページには URL を埋め込まない。/invite と同じ方式)。
// 金額 (単発の任意額、毎月のコース) は Stripe の決済ページ側で選ばせるので、ここでは扱わない

export const DONATION_LINK_ENV = {
  /** 1 回だけの支援 (Stripe Payment Links) */
  once: "STRIPE_DONATION_PAYMENT_LINK_URL",
  /** 毎月の支援 (継続課金の Stripe Payment Links) */
  monthly: "STRIPE_MONTHLY_DONATION_PAYMENT_LINK_URL",
  /** 毎月の支援の解約・カード変更 (Stripe カスタマーポータルのログインリンク) */
  portal: "STRIPE_CUSTOMER_PORTAL_URL",
} as const;

export type DonationLinkKind = keyof typeof DONATION_LINK_ENV;

/** 飛び先の URL。未設定 (空文字・空白だけを含む) なら undefined */
export function donationLinkUrl(kind: DonationLinkKind): string | undefined {
  const url = process.env[DONATION_LINK_ENV[kind]]?.trim();
  return url ? url : undefined;
}

/** ローカル開発など、飛び先を用意していない環境で返す 503 */
export function donationUnavailableResponse(kind: DonationLinkKind): Response {
  const message =
    kind === "portal"
      ? "支援の管理ページは準備中です / The donation management page is not available yet."
      : "支援の受付は準備中です / Donations are not available yet.";
  return new Response(message, {
    status: 503,
    headers: { "content-type": "text/plain; charset=utf-8" },
  });
}
