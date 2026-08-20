import type { Metadata } from "next";
import { EventCalendar } from "@/components/event-calendar";

export const metadata: Metadata = {
  title: "ダッシュボード",
};

export default function DashboardPage() {
  return (
    <main className="flex h-dvh flex-col p-4">
      <EventCalendar />
    </main>
  );
}
