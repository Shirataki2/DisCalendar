import { redirect } from "next/navigation";
import { donationLinkUrl, donationUnavailableResponse } from "@/lib/donation";

// 支援ページ (/donation) の「1 回だけ支援する」の飛び先。決済ページ (Stripe Payment Links) の URL は
// 実行時の環境変数 (STRIPE_DONATION_PAYMENT_LINK_URL) で決まり、ビルド時には分からないので、
// 静的な支援ページには埋め込まずここでリダイレクトする (/invite と同じ方式)。
// GET の Route Handler は既定で動的 (リクエストごとに評価される)
export function GET(): Response {
  const url = donationLinkUrl("once");
  if (!url) return donationUnavailableResponse("once");
  redirect(url);
}
