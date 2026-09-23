import {
  skipToken,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { ExternalCalendarInput } from "@/lib/api/types";
import type { EventRange } from "./events";
import { queryKeys } from "./keys";

export function useExternalDisplayErrors(guildId: string) {
  return useQuery<Record<number, string>>({
    queryKey: queryKeys.external.displayErrors(guildId),
    queryFn: skipToken,
  });
}

export function useExternalCalendars(guildId: string) {
  return useQuery({
    queryKey: queryKeys.guild.externalCalendars(guildId),
    queryFn: () => api.guilds.externalCalendars(guildId),
  });
}

export function useExternalEvents(
  guildId: string,
  range: EventRange | null,
  enabled = true,
) {
  const client = useQueryClient();
  return useQuery({
    queryKey: range
      ? queryKeys.external.range(guildId, range.start, range.end)
      : queryKeys.external.all(guildId),
    queryFn:
      range && enabled
        ? async ({ signal }) => {
            const result = await api.externalEvents(
              guildId,
              range.start,
              range.end,
              signal,
            );
            client.setQueryData(
              queryKeys.external.displayErrors(guildId),
              Object.fromEntries(
                result
                  .filter(({ calendar }) => calendar.last_error)
                  .map(({ calendar }) => [calendar.id, calendar.last_error]),
              ),
            );
            await client.invalidateQueries({
              queryKey: queryKeys.guild.externalCalendars(guildId),
            });
            return result;
          }
        : skipToken,
  });
}

export function useExternalCalendarActions(guildId: string) {
  const client = useQueryClient();
  const refresh = async () => {
    await Promise.all([
      client.invalidateQueries({
        queryKey: queryKeys.guild.externalCalendars(guildId),
      }),
      client.invalidateQueries({ queryKey: queryKeys.external.all(guildId) }),
    ]);
  };
  const add = useMutation({
    mutationFn: (input: ExternalCalendarInput) =>
      api.guilds.addExternalCalendar(guildId, input),
    onSuccess: refresh,
  });
  const update = useMutation({
    mutationFn: ({ id, input }: { id: number; input: ExternalCalendarInput }) =>
      api.guilds.updateExternalCalendar(guildId, id, input),
    onSuccess: refresh,
  });
  const remove = useMutation({
    mutationFn: (id: number) => api.guilds.removeExternalCalendar(guildId, id),
    onSuccess: refresh,
  });
  const fetchNow = useMutation({
    mutationFn: (id: number) => api.guilds.refreshExternalCalendar(guildId, id),
    onSuccess: refresh,
  });
  return { add, update, remove, fetchNow };
}
