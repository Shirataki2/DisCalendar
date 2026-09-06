import { describe, expect, it } from "vitest";
import { dashboardReturnPath, loginUrl } from "./login-redirect";

describe("ログイン後のカレンダーへの復帰", () => {
  it.each(["/dashboard", "/dashboard/all", "/dashboard/123456789012345678"])(
    "カレンダーのパス %s を保持する",
    (path) => {
      expect(dashboardReturnPath(path)).toBe(path);
    },
  );

  it.each([
    null,
    "",
    "https://example.com",
    "//example.com",
    "/dashboard/../admin",
    "/admin",
    "/dashboard/123?next=https://example.com",
    "/dashboard/123\n",
    "/dashboard/%2f%2fexample.com",
    "/dashboard/123456789012345678901",
  ])("許可していない戻り先 %s はサーバー一覧に戻す", (path) => {
    expect(dashboardReturnPath(path)).toBe("/dashboard");
  });

  it("通常のログイン URL は維持し、サーバーへのリンクだけ戻り先を付ける", () => {
    expect(loginUrl(null)).toBe("/login");
    expect(loginUrl("/dashboard")).toBe("/login");
    expect(loginUrl("https://example.com")).toBe("/login");
    expect(loginUrl("/dashboard/123")).toBe(
      "/login?returnTo=%2Fdashboard%2F123",
    );
  });
});
