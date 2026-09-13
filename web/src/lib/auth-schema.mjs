// Better Auth 1.7.0〜1.7.2 の必須 issuer 列と一意制約を維持する。
// 1.7.4 はこの列を書かないため、削除せずアダプターの既定値で埋める。
// このアプリの認証プロバイダーは Discord のみ。追加時は issuer の生成も見直す。
// 起動時・手動migration・E2Eで同じスキーマを使う。
/** @type {import("better-auth").BetterAuthPlugin} */
export const accountIssuerCompatibility = {
  id: "account-issuer-compatibility",
  schema: {
    account: {
      fields: {
        issuer: {
          type: "string",
          required: true,
          defaultValue: "local:oauth:discord",
          input: false,
          returned: false,
        },
      },
      indexes: [{ fields: ["issuer", "accountId"], unique: true }],
    },
  },
};
