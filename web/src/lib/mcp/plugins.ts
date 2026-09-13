import { cimd } from "@better-auth/cimd";
import { fetchClientMetadataResource } from "@better-auth/cimd/node";
import { mcp } from "@better-auth/mcp";
import { APIError } from "better-auth/api";
import { jwt } from "better-auth/plugins";
import { MCP_SCOPES, mcpResource } from "./config";
import { activeConnection, consentContext, hashOAuthToken } from "./store";

export function mcpPlugins() {
  return [
    jwt(),
    mcp({
      resource: mcpResource(),
      loginPage: "/mcp/login",
      consentPage: "/mcp/consent",
      scopes: Object.keys(MCP_SCOPES),
      grantTypes: ["authorization_code", "refresh_token"],
      accessTokenExpiresIn: 15 * 60,
      refreshTokenExpiresIn: 30 * 24 * 60 * 60,
      refreshTokenReuseInterval: 30,
      allowDynamicClientRegistration: false,
      allowUnauthenticatedClientRegistration: false,
      storeTokens: { hash: hashOAuthToken },
      postLogin: {
        page: "/mcp/consent",
        shouldRedirect: async () => !consentContext.getStore(),
        consentReferenceId: async ({ user }) => {
          const connection = consentContext.getStore();
          if (!connection || connection.userId !== user.id)
            throw new APIError("FORBIDDEN");
          return connection.id;
        },
      },
      customAccessTokenClaims: async ({
        user,
        referenceId,
        scopes,
        resources,
      }) => {
        const connection =
          user && referenceId
            ? await activeConnection(referenceId, user.id)
            : undefined;
        if (
          !connection ||
          !resources?.length ||
          !resources.every((r) => r === mcpResource()) ||
          !scopes.every((s) => connection.scopes.includes(s))
        ) {
          throw new APIError("BAD_REQUEST", { error: "invalid_grant" });
        }
        return { connection_id: connection.id };
      },
    }),
    cimd({ fetchClientMetadataResource, metadataProfile: "mcp-2026-07-28" }),
  ];
}
