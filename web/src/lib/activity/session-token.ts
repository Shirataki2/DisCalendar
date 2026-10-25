import { createHmac } from "node:crypto";

/**
 * セッショントークンを Better Auth の cookie と同じ形 `<token>.<base64(HMAC-SHA256(secret, token))>` に署名する。
 * api はこの値を `Authorization: Bearer` で受け付ける (api/src/auth.rs の verify_signed_value)。
 * Discord の Activity は第三者の iframe で cookie が届かないため、この値をブラウザのメモリに持たせて送る
 */
export function signSessionToken(token: string, secret: string): string {
  const signature = createHmac("sha256", secret).update(token).digest("base64");
  return `${token}.${signature}`;
}
