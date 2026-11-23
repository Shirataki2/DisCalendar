import { afterEach, describe, expect, it, vi } from "vitest";
import { donationLinkUrl, donationUnavailableResponse } from "./donation";

afterEach(() => {
  vi.unstubAllEnvs();
});

describe("donationLinkUrl", () => {
  it("種類ごとに別の環境変数を読む", () => {
    vi.stubEnv(
      "STRIPE_DONATION_PAYMENT_LINK_URL",
      "https://buy.stripe.com/once",
    );
    vi.stubEnv(
      "STRIPE_MONTHLY_DONATION_PAYMENT_LINK_URL",
      "https://buy.stripe.com/monthly",
    );
    vi.stubEnv(
      "STRIPE_CUSTOMER_PORTAL_URL",
      "https://billing.stripe.com/p/login/portal",
    );
    expect(donationLinkUrl("once")).toBe("https://buy.stripe.com/once");
    expect(donationLinkUrl("monthly")).toBe("https://buy.stripe.com/monthly");
    expect(donationLinkUrl("portal")).toBe(
      "https://billing.stripe.com/p/login/portal",
    );
  });

  it("未設定・空文字・空白だけなら undefined", () => {
    vi.stubEnv("STRIPE_DONATION_PAYMENT_LINK_URL", undefined);
    vi.stubEnv("STRIPE_MONTHLY_DONATION_PAYMENT_LINK_URL", "");
    vi.stubEnv("STRIPE_CUSTOMER_PORTAL_URL", "  ");
    expect(donationLinkUrl("once")).toBeUndefined();
    expect(donationLinkUrl("monthly")).toBeUndefined();
    expect(donationLinkUrl("portal")).toBeUndefined();
  });

  it("前後の空白は取り除く", () => {
    vi.stubEnv(
      "STRIPE_MONTHLY_DONATION_PAYMENT_LINK_URL",
      " https://buy.stripe.com/monthly\n",
    );
    expect(donationLinkUrl("monthly")).toBe("https://buy.stripe.com/monthly");
  });
});

describe("donationUnavailableResponse", () => {
  it("503 と「準備中」を返す", async () => {
    const response = donationUnavailableResponse("monthly");
    expect(response.status).toBe(503);
    expect(response.headers.get("content-type")).toContain("text/plain");
    expect(await response.text()).toContain("支援の受付は準備中です");
  });

  it("管理ページは文言を分ける", async () => {
    const response = donationUnavailableResponse("portal");
    expect(response.status).toBe(503);
    expect(await response.text()).toContain("支援の管理ページは準備中です");
  });
});
