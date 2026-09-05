import { type NextRequest, NextResponse } from "next/server";
import { RETURN_TO_HEADER } from "@/lib/login-redirect";

export function proxy(request: NextRequest) {
  // レイアウトには子ページのパスが渡らないため、実際のパスで必ず上書きして渡す。
  // 認証・認可は従来どおりセッション検証と API に任せる。
  const headers = new Headers(request.headers);
  headers.set(RETURN_TO_HEADER, request.nextUrl.pathname);
  return NextResponse.next({ request: { headers } });
}

export const config = { matcher: "/dashboard/:path*" };
