"use client";

import { format, parseISO } from "date-fns";
import { ja } from "date-fns/locale";
import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { describeApiError } from "@/lib/api";
import type { RecurrenceEnding, RecurrenceRule } from "@/lib/api/types";
import { describeRecurrence, WEEKDAYS } from "@/lib/recurrence";

const selectClass =
  "min-h-11 w-full rounded-md border bg-background px-3 text-base focus-visible:outline-2 focus-visible:outline-ring";
interface Props {
  value: RecurrenceRule;
  start: string;
  preview: (
    start: string,
    rule: RecurrenceRule,
    signal: AbortSignal,
  ) => Promise<string[]>;
  onApply: (rule: RecurrenceRule) => void;
  onCancel: () => void;
  onDirty: (dirty: boolean) => void;
}
export function RecurrenceSettings({
  value,
  start,
  preview,
  onApply,
  onCancel,
  onDirty,
}: Props) {
  const [rule, setRule] = useState<RecurrenceRule>(value);
  const [dates, setDates] = useState<string[]>([]);
  const [error, setError] = useState<string>();
  const [loading, setLoading] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    heading.current?.focus();
  }, []);
  useEffect(() => {
    onDirty(JSON.stringify(rule) !== JSON.stringify(value));
  }, [rule, value, onDirty]);
  useEffect(() => {
    const controller = new AbortController();
    if (rule.frequency === "none") {
      setDates([]);
      setError(undefined);
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(undefined);
    const timer = setTimeout(() => {
      preview(start, rule, controller.signal)
        .then((result) => {
          if (!controller.signal.aborted) setDates(result);
        })
        .catch((cause) => {
          if (!controller.signal.aborted) {
            setError(describeApiError(cause));
            setDates([]);
          }
        })
        .finally(() => {
          if (!controller.signal.aborted) setLoading(false);
        });
    }, 200);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [start, rule, preview]);
  const end: RecurrenceEnding =
    rule.frequency === "none" ? { type: "never" } : rule.end;
  const startDate = parseISO(start);
  const weekday = (startDate.getDay() + 6) % 7;
  function choose(frequency: RecurrenceRule["frequency"]) {
    if (frequency === "none") setRule({ frequency });
    else if (frequency === "daily") setRule({ frequency, end });
    else if (frequency === "weekly" || frequency === "biweekly")
      setRule({ frequency, weekdays: [weekday], end });
    else if (frequency === "monthly_date")
      setRule({ frequency, day: startDate.getDate(), end });
    else
      setRule({
        frequency,
        nth: Math.floor((startDate.getDate() - 1) / 7) + 1,
        weekday,
        end,
      });
  }
  function changeEnd(end: RecurrenceEnding) {
    if (rule.frequency !== "none") setRule({ ...rule, end });
  }
  return (
    <div className="flex min-h-0 flex-col">
      <DialogHeader className="shrink-0 border-b p-4 pr-14">
        <Button
          type="button"
          variant="ghost"
          className="w-fit"
          onClick={onCancel}
        >
          ← 予定に戻る
        </Button>
        <DialogTitle ref={heading} tabIndex={-1}>
          繰り返しの設定
        </DialogTitle>
        <DialogDescription>
          条件と開催日を確認して、予定のフォームに適用します。
        </DialogDescription>
      </DialogHeader>
      <div className="min-h-0 space-y-5 overflow-y-auto p-4">
        <label className="grid gap-2">
          繰り返しの頻度
          <select
            className={selectClass}
            value={rule.frequency}
            onChange={(e) =>
              choose(e.target.value as RecurrenceRule["frequency"])
            }
          >
            <option value="none">繰り返しなし</option>
            <option value="daily">毎日</option>
            <option value="weekly">毎週</option>
            <option value="biweekly">隔週</option>
            <option value="monthly_date">毎月（日付）</option>
            <option value="monthly_weekday">毎月（第n曜日）</option>
          </select>
        </label>
        {(rule.frequency === "weekly" || rule.frequency === "biweekly") && (
          <fieldset className="space-y-2">
            <legend>曜日（複数選択）</legend>
            <div className="flex flex-wrap gap-2">
              {WEEKDAYS.map((day, index) => (
                <Button
                  key={day}
                  type="button"
                  className="min-h-11 min-w-11"
                  aria-pressed={rule.weekdays.includes(index)}
                  variant={
                    rule.weekdays.includes(index) ? "default" : "outline"
                  }
                  onClick={() =>
                    setRule({
                      ...rule,
                      weekdays: rule.weekdays.includes(index)
                        ? rule.weekdays.filter((d) => d !== index)
                        : [...rule.weekdays, index].sort(),
                    })
                  }
                >
                  {day}
                </Button>
              ))}
            </div>
          </fieldset>
        )}
        {rule.frequency === "monthly_date" && (
          <label className="grid gap-2" htmlFor="recurrence-month-day">
            毎月の日付
            <Input
              id="recurrence-month-day"
              type="number"
              min={1}
              max={31}
              value={rule.day}
              onChange={(e) =>
                setRule({ ...rule, day: Number(e.target.value) })
              }
            />
          </label>
        )}
        {rule.frequency === "monthly_weekday" && (
          <div className="grid grid-cols-2 gap-3">
            <label className="grid gap-2">
              週の順番
              <select
                className={selectClass}
                value={rule.nth}
                onChange={(e) =>
                  setRule({ ...rule, nth: Number(e.target.value) })
                }
              >
                {[1, 2, 3, 4, 5].map((nth) => (
                  <option key={nth} value={nth}>
                    第{nth}
                  </option>
                ))}
              </select>
            </label>
            <label className="grid gap-2">
              曜日
              <select
                className={selectClass}
                value={rule.weekday}
                onChange={(e) =>
                  setRule({ ...rule, weekday: Number(e.target.value) })
                }
              >
                {WEEKDAYS.map((day, index) => (
                  <option key={day} value={index}>
                    {day}曜日
                  </option>
                ))}
              </select>
            </label>
          </div>
        )}
        {(rule.frequency === "monthly_date" ||
          rule.frequency === "monthly_weekday") && (
          <p className="text-sm text-muted-foreground">
            該当日がない月はスキップします。開始日と同じ日付・第n曜日を指定してください。
          </p>
        )}
        {rule.frequency !== "none" && (
          <fieldset className="space-y-3">
            <legend className="mb-2">終了条件</legend>
            <label className="flex min-h-11 items-center gap-2">
              <input
                type="radio"
                name="recurrence-ending"
                checked={end.type === "never"}
                onChange={() => changeEnd({ type: "never" })}
              />
              終了なし
            </label>
            <label className="flex min-h-11 items-center gap-2">
              <input
                type="radio"
                name="recurrence-ending"
                checked={end.type === "until"}
                onChange={() =>
                  changeEnd({ type: "until", date: start.slice(0, 10) })
                }
              />
              終了日
            </label>
            {end.type === "until" && (
              <Input
                aria-label="繰り返しの終了日"
                type="date"
                min={start.slice(0, 10)}
                value={end.date}
                onChange={(e) =>
                  changeEnd({ type: "until", date: e.target.value })
                }
              />
            )}
            <label className="flex min-h-11 items-center gap-2">
              <input
                type="radio"
                name="recurrence-ending"
                checked={end.type === "count"}
                onChange={() => changeEnd({ type: "count", count: 8 })}
              />
              回数
            </label>
            {end.type === "count" && (
              <Input
                aria-label="繰り返し回数"
                type="number"
                min={1}
                max={10000}
                value={end.count}
                onChange={(e) =>
                  changeEnd({ type: "count", count: Number(e.target.value) })
                }
              />
            )}
          </fieldset>
        )}
        <div className="rounded-lg border p-3" aria-live="polite">
          <p className="font-medium">{describeRecurrence(rule)}</p>
          {loading ? (
            <p>開催日を確認中…</p>
          ) : error ? (
            <p role="alert" className="text-destructive">
              {error}
            </p>
          ) : (
            dates.length > 0 && (
              <>
                <p className="mt-3 text-sm text-muted-foreground">
                  次の開催日（日本時間）
                </p>
                <ul>
                  {dates.map((date) => (
                    <li key={date}>
                      {format(parseISO(date), "yyyy/MM/dd (EEE) HH:mm", {
                        locale: ja,
                      })}
                    </li>
                  ))}
                </ul>
              </>
            )
          )}
        </div>
      </div>
      <DialogFooter className="m-0 shrink-0 flex-row justify-end [&>button]:min-h-11">
        <Button type="button" variant="outline" onClick={onCancel}>
          キャンセル
        </Button>
        <Button
          type="button"
          disabled={loading || !!error}
          onClick={() => onApply(rule)}
        >
          設定を適用
        </Button>
      </DialogFooter>
    </div>
  );
}
