import { addDays } from "date-fns";
import type { ApiEvent, ApiEventInput } from "./api/types";
import { toApiDateTime } from "./calendar-events";
import {
  defaultEventFormValues,
  eventFormSchema,
  eventFormToApiInput,
  eventToFormValues,
} from "./event-form";
import type { EventsSource } from "./query/events";

// Snowflake ではない ID と独立したキーで、実サーバーと混ざらないようにする。
export const TUTORIAL_GUILD_ID = "tutorial";
export type TutorialAction = "create" | "update" | "remove";

/** ページを開くたびに生成する。通信・永続化は行わない。 */
export function createTutorialEventsSource(
  onChange: (action: TutorialAction, event: ApiEvent) => void,
  now = new Date(),
): EventsSource {
  let nextId = 1;
  const makeEvent = (input: ApiEventInput, id: number): ApiEvent => {
    const { discord_scheduled_event: _sync, ...values } = input;
    const event: ApiEvent = {
      ...values,
      id,
      guild_id: TUTORIAL_GUILD_ID,
      description: input.description ?? null,
      location: input.location ?? null,
      notification_mentions: [],
      discord_scheduled_event_id: null,
      created_at: toApiDateTime(now),
      created_by: null,
      updated_by: null,
      updated_at: null,
    };
    // ドラッグなど、フォームを経由しない操作も同じ入力規則で検証する。
    eventFormSchema.parse(eventToFormValues(event));
    return event;
  };
  let events = [
    { name: "みんなでゲーム会", date: now, color: "#5865F2" },
    { name: "週末の作戦会議", date: addDays(now, 1), color: "#009688" },
  ].map(({ name, date, color }) =>
    makeEvent(
      eventFormToApiInput({
        ...defaultEventFormValues(date),
        name,
        color,
        description: "これは練習用の予定です。自由に編集してみましょう。",
      }),
      nextId++,
    ),
  );

  const checkGuild = (guildId: string) => {
    if (guildId !== TUTORIAL_GUILD_ID)
      throw new Error("練習用サーバーを選んでください");
  };
  const findEvent = (id: number) => {
    const event = events.find((item) => item.id === id);
    if (!event)
      throw new Error("予定が見つかりません。別の予定を選んでください");
    return event;
  };
  return {
    keys: {
      all: (guildId) => ["tutorial", "events", guildId],
      range: (guildId, start, end) => [
        "tutorial",
        "events",
        guildId,
        { start, end },
      ],
    },
    client: {
      list: async (guildId, start, end) => {
        checkGuild(guildId);
        // API と同じく end_at を含める。終日予定の終了日は翌日ではなく当日。
        return structuredClone(
          events.filter(
            (event) => event.start_at < end && event.end_at >= start,
          ),
        );
      },
      create: async (guildId, input) => {
        checkGuild(guildId);
        const event = makeEvent(input, nextId++);
        events.push(event);
        onChange("create", structuredClone(event));
        return structuredClone(event);
      },
      update: async (guildId, id, input) => {
        checkGuild(guildId);
        const previous = findEvent(id);
        const event = {
          ...makeEvent(input, id),
          created_at: previous.created_at,
          updated_at: toApiDateTime(new Date()),
        };
        events = events.map((item) => (item.id === id ? event : item));
        onChange("update", structuredClone(event));
        return structuredClone(event);
      },
      remove: async (guildId, id) => {
        checkGuild(guildId);
        const event = findEvent(id);
        events = events.filter((item) => item.id !== id);
        onChange("remove", structuredClone(event));
      },
    },
  };
}
