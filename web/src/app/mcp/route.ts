import { createMcpHandler, McpServer } from "@modelcontextprotocol/server";
import { z } from "zod";
import { protectedMcp } from "@/lib/mcp/protected";
import pkg from "../../../package.json";

export const runtime = "nodejs";
export async function POST(request: Request) {
  return protectedMcp(request, async (req, connection, claims) => {
    const handler = createMcpHandler(() => {
      const server = new McpServer({
        name: "discalendar",
        version: pkg.version,
      });
      server.registerTool(
        "connection_info",
        {
          description:
            "この接続で許可したサーバーIDと操作権限を確認します。予定やDiscordの認証情報は返しません。",
          inputSchema: z.object({}).strict(),
          annotations: {
            readOnlyHint: true,
            destructiveHint: false,
            openWorldHint: false,
          },
        },
        async () => ({
          content: [
            {
              type: "text",
              text: JSON.stringify({
                connection_id: connection.id,
                guild_ids: connection.guild_ids,
                scope: claims.scope,
              }),
            },
          ],
        }),
      );
      return server;
    });
    return handler.fetch(req);
  });
}
