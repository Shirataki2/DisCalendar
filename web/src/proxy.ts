import { type NextRequest, NextResponse } from "next/server";
import { RETURN_TO_HEADER } from "@/lib/login-redirect";

export function proxy(request: NextRequest) {
  const { pathname, search, searchParams } = request.nextUrl;
  // Discord の Activity (#350) はアプリのルートを frame_id / instance_id 付きで開く。
  // SDK がこのクエリを読むので URL は変えずに Activity の画面へ rewrite する
  // (redirect だと Location が公開 URL になり、Discord のプロキシから外れる)
  if (pathname === "/") {
    if (searchParams.has("frame_id") && searchParams.has("instance_id")) {
      const url = request.nextUrl.clone();
      url.pathname = "/activity";
      return NextResponse.rewrite(url);
    }
    return NextResponse.next();
  }

  // レイアウトには子ページのパスが渡らないため、実際のパスで必ず上書きして渡す。
  // 認証・認可は従来どおりセッション検証と API に任せる。
  const headers = new Headers(request.headers);
  headers.set(RETURN_TO_HEADER, `${pathname}${search}`);
  return NextResponse.next({ request: { headers } });
}

export const config = { matcher: ["/", "/dashboard/:path*"] };
