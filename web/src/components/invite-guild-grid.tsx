"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import {
  GuildCardBody,
  GuildGrid,
  guildCardClassName,
} from "@/components/guild-card";
import { api } from "@/lib/api";

export interface InvitableGuild {
  id: string;
  name: string;
  iconUrl: string | null;
  /** Discord の Bot 追加画面 (lib/discord.ts の botInviteUrl) */
  inviteUrl: string;
}

/**
 * 参加状況が空振りだったときに試し直す間隔 (ms)。招待を終えてすぐ戻ってくると、
 * Bot が Discord のイベントを受けて guilds テーブルに書き終える前に問い合わせることがある
 */
const RETRY_DELAYS = [1000, 2000, 4000];

/**
 * Bot を招待できるサーバーの一覧。
 * 招待は Discord の Bot 追加画面を別タブで開くので、戻ってきただけでは Server Component の
 * この画面は「参加済み」に変わらない。招待画面を開いたサーバーを覚えておき、タブに戻ったときに
 * 参加状況を問い合わせて、参加していればそのサーバーのカレンダーへ移動する。
 * (旧実装はポップアップが閉じたら参加を確かめずに移動していたので、途中でやめると開けない画面に飛んでいた)
 */
export function InviteGuildGrid({ guilds }: { guilds: InvitableGuild[] }) {
  const router = useRouter();
  /** 招待画面を開いたサーバー。戻ってきたときの問い合わせ対象 */
  const [invited, setInvited] = useState<string[]>([]);
  const [checking, setChecking] = useState(false);

  useEffect(() => {
    if (invited.length === 0) return;
    // 招待したサーバーが増えるとこの effect は作り直される。作り直す前に投げた問い合わせの
    // 結果で移動してしまわないよう、古い方は打ち切る
    let cancelled = false;
    let running = false;

    const check = async () => {
      if (cancelled || running) return;
      running = true;
      setChecking(true);
      try {
        for (let attempt = 0; !cancelled; attempt++) {
          const joined = await api.guilds.joined(invited);
          if (cancelled) return;
          if (joined.length > 0) {
            const joinedIds = new Set(joined.map((guild) => guild.guild_id));
            setInvited((ids) => ids.filter((id) => !joinedIds.has(id)));
            if (joined.length === 1) {
              // 招待が済んだサーバーが 1 つならそのカレンダーへ (旧実装と同じ体験)
              router.push(`/dashboard/${joined[0].guild_id}`);
            } else {
              // 複数まとめて招待した場合はどれを開くべきか決められないので、一覧の更新だけにする
              router.refresh();
            }
            return;
          }
          // Bot の参加がまだ DB に届いていないだけかもしれないので、間を置いて試し直す
          const delay = RETRY_DELAYS[attempt];
          if (delay === undefined) return;
          await new Promise((resolve) => setTimeout(resolve, delay));
        }
      } catch {
        // 参加状況を取れなくても画面はそのまま (次に戻ってきたときに試し直す)
      } finally {
        running = false;
        if (!cancelled) setChecking(false);
      }
    };
    const onReturn = () => {
      if (document.visibilityState === "visible") void check();
    };
    // 別タブ / ポップアップから戻ったことを、タブの切り替え (visibilitychange) と
    // ウィンドウのフォーカス (focus) の両方で拾う。どちらが起きるかは招待画面の閉じ方で変わる
    document.addEventListener("visibilitychange", onReturn);
    window.addEventListener("focus", onReturn);
    return () => {
      cancelled = true;
      document.removeEventListener("visibilitychange", onReturn);
      window.removeEventListener("focus", onReturn);
    };
  }, [invited, router]);

  return (
    <GuildGrid>
      {guilds.map((guild) => {
        // 中クリックで開いた場合は click ではなく auxclick なので、両方で覚える
        const remember = () =>
          setInvited((ids) =>
            ids.includes(guild.id) ? ids : [...ids, guild.id],
          );
        return (
          <li key={guild.id}>
            {/* 招待は別タブで開く (ポップアップブロックを避けるため window.open は使わない) */}
            <a
              href={guild.inviteUrl}
              target="_blank"
              rel="noopener noreferrer"
              className={guildCardClassName(true)}
              onClick={remember}
              onAuxClick={remember}
            >
              <GuildCardBody
                name={guild.name}
                iconUrl={guild.iconUrl}
                badge={
                  checking && invited.includes(guild.id) ? "確認中…" : "招待 ↗"
                }
              />
            </a>
          </li>
        );
      })}
    </GuildGrid>
  );
}
