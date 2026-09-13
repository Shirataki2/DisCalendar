import { auth } from "@/lib/auth";
import { noStore } from "@/lib/mcp/config";
import { boundedRequest, sameOrigin } from "@/lib/mcp/http";
import { revokeConnections } from "@/lib/mcp/store";

export async function POST(request: Request) {
  if (!sameOrigin(request))
    return new Response(null, { status: 403, headers: noStore });
  const session = await auth.api.getSession({ headers: request.headers });
  if (!session) return new Response(null, { status: 401, headers: noStore });
  let id: string;
  try {
    id = String(
      (await (await boundedRequest(request)).formData()).get("id") ?? "",
    );
  } catch {
    return new Response(null, { status: 400, headers: noStore });
  }
  if (!/^[a-f0-9-]{36}$/.test(id))
    return new Response(null, { status: 400, headers: noStore });
  await revokeConnections(session.user.id, id);
  return new Response(null, {
    status: 303,
    headers: { ...noStore, Location: "/mcp/connections" },
  });
}
