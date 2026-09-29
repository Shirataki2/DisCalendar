import { notFound } from "next/navigation";
import { PollList } from "@/components/schedule-polls";
export const metadata = { title: "日程調整" };
export default async function Page({
  params,
}: PageProps<"/dashboard/[id]/polls">) {
  const { id } = await params;
  if (!/^\d{1,20}$/.test(id)) notFound();
  return <PollList key={id} guildId={id} />;
}
