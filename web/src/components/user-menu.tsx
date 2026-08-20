"use client";

import { useRouter } from "next/navigation";
import { authClient } from "@/lib/auth-client";

interface Props {
  name: string;
  image: string | null;
}

export function UserMenu({ name, image }: Props) {
  const router = useRouter();

  const signOut = async () => {
    await authClient.signOut();
    router.push("/login");
  };

  return (
    <div className="flex items-center gap-3">
      {image && (
        // biome-ignore lint/performance/noImgElement: Discord CDN のアバターは最適化不要
        <img src={image} alt="" className="h-8 w-8 rounded-full" />
      )}
      <span className="text-sm font-medium">{name}</span>
      <button
        type="button"
        onClick={signOut}
        className="rounded-full border border-white/20 px-4 py-1.5 text-xs text-neutral-300 transition-colors hover:bg-white/10"
      >
        ログアウト
      </button>
    </div>
  );
}
