import { headers } from "next/headers";
import { redirect } from "next/navigation";
import { auth } from "@/lib/auth";
import { loginUrl, RETURN_TO_HEADER } from "@/lib/login-redirect";

export async function requireSession() {
  const requestHeaders = await headers();
  const session = await auth.api.getSession({ headers: requestHeaders });
  if (!session) {
    redirect(loginUrl(requestHeaders.get(RETURN_TO_HEADER)));
  }
  return session;
}
