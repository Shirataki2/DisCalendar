"use client";

import { useState } from "react";
import { ColorPicker } from "@/components/form/color-picker";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { describeApiError } from "@/lib/api";
import type { ExternalCalendar, ExternalCalendarInput } from "@/lib/api/types";
import { DEFAULT_COLOR } from "@/lib/event-form";
import {
  useExternalCalendarActions,
  useExternalCalendars,
  useExternalDisplayErrors,
} from "@/lib/query/external-calendars";

export function ExternalCalendarSettings({
  guildId,
  canManage,
}: {
  guildId: string;
  canManage: boolean;
}) {
  const calendars = useExternalCalendars(guildId);
  const displayErrors = useExternalDisplayErrors(guildId);
  const actions = useExternalCalendarActions(guildId);
  const [editing, setEditing] = useState<number | null>(null);
  const [input, setInput] = useState<ExternalCalendarInput>({
    url: "",
    name: "",
    color: DEFAULT_COLOR,
  });
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const beginEdit = (calendar: ExternalCalendar) => {
    if (!calendar.url) return;
    setEditing(calendar.id);
    setInput({ url: calendar.url, name: calendar.name, color: calendar.color });
    setError(null);
  };
  const reset = () => {
    setEditing(null);
    setInput({ url: "", name: "", color: DEFAULT_COLOR });
    setError(null);
  };
  const submit = async () => {
    setError(null);
    setBusy(true);
    try {
      if (editing === null) await actions.add.mutateAsync(input);
      else await actions.update.mutateAsync({ id: editing, input });
      reset();
    } catch (cause) {
      setError(describeApiError(cause));
    } finally {
      setBusy(false);
    }
  };
  const run = async (action: () => Promise<unknown>) => {
    setError(null);
    setBusy(true);
    try {
      await action();
    } catch (cause) {
      setError(describeApiError(cause));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section aria-labelledby="external-calendar-heading" className="grid gap-3">
      <div>
        <h3 id="external-calendar-heading" className="text-sm font-medium">
          外部カレンダーを重ねて表示する
        </h3>
        <p className="text-sm text-muted-foreground">
          登録した ICS
          の予定を読み取り専用で表示します。通知や共有には含まれません
        </p>
      </div>
      {calendars.isPending ? (
        <p className="text-sm text-muted-foreground">確認中…</p>
      ) : calendars.isError ? (
        <p role="alert" className="text-sm text-destructive">
          {describeApiError(calendars.error)}
        </p>
      ) : (
        <ul className="grid gap-2" aria-label="登録済みの外部カレンダー">
          {calendars.data.map((calendar) => (
            <li key={calendar.id} className="rounded-md border p-3 text-sm">
              <div className="flex flex-wrap items-center gap-2">
                <span
                  aria-hidden
                  className="size-3 shrink-0 rounded-full"
                  style={{ backgroundColor: calendar.color }}
                />
                <strong className="min-w-0 flex-1 break-words">
                  {calendar.name}
                </strong>
                {canManage && (
                  <>
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      disabled={busy}
                      onClick={() => beginEdit(calendar)}
                    >
                      編集
                    </Button>
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      disabled={busy}
                      onClick={() =>
                        void run(() =>
                          actions.fetchNow.mutateAsync(calendar.id),
                        )
                      }
                    >
                      今すぐ取得
                    </Button>
                    <Button
                      type="button"
                      size="sm"
                      variant="destructive"
                      disabled={busy}
                      onClick={() => {
                        if (
                          window.confirm(
                            `「${calendar.name}」の購読を削除しますか？`,
                          )
                        )
                          void run(() =>
                            actions.remove.mutateAsync(calendar.id),
                          );
                      }}
                    >
                      削除
                    </Button>
                  </>
                )}
              </div>
              {canManage && calendar.url && (
                <p className="mt-1 break-all text-xs text-muted-foreground">
                  {calendar.url}
                </p>
              )}
              <p className="mt-1 text-xs text-muted-foreground">
                最終取得:{" "}
                {calendar.last_fetched_at
                  ? calendar.last_fetched_at.replace("T", " ")
                  : "まだ取得していません"}
              </p>
              {displayErrors.data?.[calendar.id] &&
                displayErrors.data[calendar.id] !== calendar.last_error && (
                  <p role="status" className="mt-1 text-xs text-destructive">
                    表示中の期間を展開できません:{" "}
                    {displayErrors.data[calendar.id]}
                  </p>
                )}
              {calendar.last_error && (
                <p role="status" className="mt-1 text-xs text-destructive">
                  取得できません: {calendar.last_error}
                </p>
              )}
            </li>
          ))}
        </ul>
      )}
      {(canManage && (calendars.data?.length ?? 0) < 5) || editing !== null ? (
        <div className="grid gap-2 rounded-md border p-3">
          <p className="text-sm font-medium">
            {editing === null ? "外部カレンダーを追加" : "外部カレンダーを編集"}
          </p>
          <label
            htmlFor="external-calendar-name"
            className="grid gap-1 text-sm"
          >
            表示名
            <Input
              id="external-calendar-name"
              value={input.name}
              maxLength={32}
              required
              disabled={busy}
              onChange={(event) =>
                setInput({ ...input, name: event.target.value })
              }
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                  event.preventDefault();
                  if (input.name.trim() && input.url.trim()) void submit();
                }
              }}
            />
          </label>
          <label htmlFor="external-calendar-url" className="grid gap-1 text-sm">
            ICS URL
            <Input
              id="external-calendar-url"
              type="url"
              value={input.url}
              required
              disabled={busy}
              placeholder="https://example.com/calendar.ics"
              onChange={(event) =>
                setInput({ ...input, url: event.target.value })
              }
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                  event.preventDefault();
                  if (input.name.trim() && input.url.trim()) void submit();
                }
              }}
            />
          </label>
          <div className="grid gap-1 text-sm">
            <span>表示色</span>
            <ColorPicker
              value={input.color}
              onChange={(color) => setInput({ ...input, color })}
            />
          </div>
          <div className="flex justify-end gap-2">
            {editing !== null && (
              <Button
                type="button"
                variant="outline"
                disabled={busy}
                onClick={reset}
              >
                キャンセル
              </Button>
            )}
            <Button
              type="button"
              disabled={busy || !input.name.trim() || !input.url.trim()}
              onClick={() => void submit()}
            >
              {busy ? "保存中…" : editing === null ? "追加" : "保存"}
            </Button>
          </div>
        </div>
      ) : null}
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {calendars.data?.length === 5 && editing === null && canManage && (
        <p className="text-xs text-muted-foreground">
          外部カレンダーは 5 件まで登録できます
        </p>
      )}
    </section>
  );
}
