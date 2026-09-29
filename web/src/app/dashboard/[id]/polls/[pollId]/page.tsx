import { notFound } from "next/navigation";
import { PollPage } from "@/components/schedule-polls";
export const metadata = { title: "日程調整への回答" };
export default async function Page({
  params,
  searchParams,
}: PageProps<"/dashboard/[id]/polls/[pollId]">) {
  const { id, pollId } = await params;
  const { announcement } = await searchParams;
  if (
    !/^\d{1,20}$/.test(id) ||
    !/^[1-9]\d*$/.test(pollId) ||
    Number(pollId) > 2147483647
  )
    notFound();
  return (
    <PollPage
      key={`${id}/${pollId}`}
      guildId={id}
      pollId={Number(pollId)}
      initialAnnouncement={
        announcement === "sent" ||
        announcement === "failed" ||
        announcement === "not_configured"
          ? announcement
          : undefined
      }
    />
  );
}
