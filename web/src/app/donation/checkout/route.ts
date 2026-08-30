import { redirect } from "next/navigation";

// 支援ページ (/donation) の「DisCalendar を支援する」の飛び先。決済ページ (Stripe Payment Links) の URL は
// 実行時の環境変数で決まり、ビルド時には分からないので、静的な支援ページには埋め込まずここでリダイレクトする
// (/invite と同じ方式)。GET の Route Handler は既定で動的 (リクエストごとに評価される)
export function GET(): Response {
  const url = process.env.STRIPE_DONATION_PAYMENT_LINK_URL;
  if (!url) {
    // ローカル開発など、支援の受付を用意していない環境
    return new Response("支援の受付は準備中です", {
      status: 503,
      headers: { "content-type": "text/plain; charset=utf-8" },
    });
  }
  redirect(url);
}
