import { describe, expect, it } from "vitest";
import { signSessionToken } from "./session-token";

describe("signSessionToken", () => {
  it("api の単体テスト (api/src/auth.rs) と同じ署名付き値を作る", () => {
    expect(signSessionToken("abc", "test-secret")).toBe(
      "abc.5MTa3WVx+Bp7AZ0hbbHvOHewyI+qvFamb4iX7jKFdZM=",
    );
  });
});
