"use client";

import { CalendarPlusIcon } from "lucide-react";
import { useMemo, useState } from "react";
import { ColorPicker } from "@/components/form/color-picker";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { api, describeApiError } from "@/lib/api";
import type {
  ImportPreview,
  ImportSkipped,
  RecurrenceRule,
} from "@/lib/api/types";
import { useImportEvents } from "@/lib/query/events";

const FILE_MAX_BYTES = 1024 * 1024;
const OCCURRENCE_MAX = 10_000;

const SKIP_LABELS: Record<ImportSkipped["reason"], string> = {
  invalid_event: "必要な項目がない、または形式が不正",
  recurrence_exceptions: "個別変更・除外日を含む繰り返し",
  cancelled: "キャンセル済み",
  unsupported_recurrence: "対応していない繰り返し条件",
  missing_title: "タイトルなし",
  invalid_date: "日時が不正",
  unknown_timezone: "タイムゾーンを解決できない",
};

function recurrenceLabel(rule: RecurrenceRule | undefined) {
  if (!rule || rule.frequency === "none") return null;
  return {
    daily: "毎日",
    weekly: "毎週",
    biweekly: "隔週",
    monthly_date: "毎月",
    monthly_weekday: "毎月",
  }[rule.frequency];
}

function formatDateTime(value: string, allDay: boolean) {
  return allDay ? value.slice(0, 10) : value.replace("T", " ").slice(0, 16);
}

interface Props {
  guildId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  defaultColor: string;
}

export function IcsImportDialog({
  guildId,
  open,
  onOpenChange,
  defaultColor,
}: Props) {
  const [contents, setContents] = useState("");
  const [filename, setFilename] = useState("");
  const [startDate, setStartDate] = useState("");
  const [endDate, setEndDate] = useState("");
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [color, setColor] = useState(defaultColor);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [importedCount, setImportedCount] = useState<number | null>(null);
  const importEvents = useImportEvents(guildId);

  const selectedItems = useMemo(
    () =>
      preview?.items.filter((item) => selected.has(item.source_index)) ?? [],
    [preview, selected],
  );
  const selectedOccurrences = selectedItems.reduce(
    (sum, item) => sum + item.estimated_occurrences,
    0,
  );

  function reset() {
    setContents("");
    setFilename("");
    setStartDate("");
    setEndDate("");
    setPreview(null);
    setSelected(new Set());
    setColor(defaultColor);
    setError(null);
    setImportedCount(null);
  }

  async function loadPreview(ics = contents, range = true) {
    if (!ics) return;
    setLoading(true);
    setError(null);
    setImportedCount(null);
    try {
      const result = await api.events.previewImport(
        guildId,
        ics,
        range
          ? {
              ...(startDate ? { start_date: startDate } : {}),
              ...(endDate ? { end_date: endDate } : {}),
            }
          : undefined,
      );
      setPreview(result);
      setSelected(
        new Set(
          result.items
            .filter((item) => !item.duplicate)
            .map((item) => item.source_index),
        ),
      );
    } catch (cause) {
      setPreview(null);
      setSelected(new Set());
      setError(describeApiError(cause));
    } finally {
      setLoading(false);
    }
  }

  async function chooseFile(file: File | undefined) {
    if (!file) return;
    setError(null);
    if (file.size > FILE_MAX_BYTES) {
      setError("ICS ファイルは 1 MiB 以下にしてください");
      return;
    }
    try {
      const text = await file.text();
      setContents(text);
      setFilename(file.name);
      setStartDate("");
      setEndDate("");
      await loadPreview(text, false);
    } catch {
      setError("ファイルを読み取れませんでした");
    }
  }

  async function submit() {
    setError(null);
    setImportedCount(null);
    try {
      await importEvents.mutateAsync(
        selectedItems.map(({ event }) => ({ ...event, color })),
      );
      setImportedCount(selectedItems.length);
      setSelected(new Set());
    } catch (cause) {
      setError(describeApiError(cause));
    }
  }

  const blocked =
    loading ||
    importEvents.isPending ||
    !selectedItems.length ||
    !!preview?.over_event_limit ||
    selectedOccurrences > OCCURRENCE_MAX;

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        onOpenChange(next);
        if (!next) reset();
      }}
    >
      <DialogContent className="h-[min(46rem,calc(100dvh-2rem))] max-w-3xl grid-rows-[auto_1fr_auto] gap-0 p-0 sm:max-w-3xl">
        <DialogHeader className="border-b p-4 pr-12">
          <DialogTitle>ICSファイルから取り込む</DialogTitle>
          <DialogDescription>
            外部カレンダーの予定を確認し、選んだものをこのサーバーへ追加します。
          </DialogDescription>
        </DialogHeader>

        <div className="min-h-0 space-y-4 overflow-y-auto p-4">
          <div className="space-y-2">
            <Label htmlFor="ics-import-file">ICSファイル</Label>
            <Input
              id="ics-import-file"
              type="file"
              accept=".ics,text/calendar"
              disabled={loading || importEvents.isPending}
              onChange={(event) => void chooseFile(event.target.files?.[0])}
              className="min-h-11 file:mr-3 file:font-medium"
            />
            <p className="text-xs text-muted-foreground">
              {filename || "1 MiBまで。ファイルは保存されません。"}
            </p>
          </div>

          {preview && (
            <>
              <div className="grid gap-3 rounded-lg border bg-muted/30 p-3 sm:grid-cols-[1fr_1fr_auto] sm:items-end">
                <div className="space-y-1.5">
                  <Label htmlFor="ics-import-start">開始日</Label>
                  <Input
                    id="ics-import-start"
                    type="date"
                    value={startDate}
                    min={preview.available_start_date ?? undefined}
                    max={endDate || preview.available_end_date || undefined}
                    onChange={(event) => setStartDate(event.target.value)}
                  />
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="ics-import-end">終了日</Label>
                  <Input
                    id="ics-import-end"
                    type="date"
                    value={endDate}
                    min={startDate || preview.available_start_date || undefined}
                    max={preview.available_end_date ?? undefined}
                    onChange={(event) => setEndDate(event.target.value)}
                  />
                </div>
                <Button
                  type="button"
                  variant="outline"
                  className="min-h-11"
                  disabled={loading}
                  onClick={() => void loadPreview()}
                >
                  期間を反映
                </Button>
              </div>

              <div className="flex flex-wrap items-end justify-between gap-3">
                <div>
                  <p className="font-medium">
                    {preview.matched_count}件中 {selectedItems.length}件を選択
                  </p>
                  <p className="text-xs text-muted-foreground">
                    生成見込み {selectedOccurrences.toLocaleString("ja-JP")}件
                  </p>
                </div>
                <div className="w-36 space-y-1.5">
                  <Label htmlFor="ics-import-color">取り込み色</Label>
                  <ColorPicker
                    id="ics-import-color"
                    value={color}
                    onChange={setColor}
                  />
                </div>
              </div>

              {preview.over_event_limit && (
                <p
                  role="alert"
                  className="rounded-md bg-amber-500/15 p-3 text-sm"
                >
                  一度に取り込めるのは200件までです。期間を狭めてください。
                </p>
              )}
              {selectedOccurrences > OCCURRENCE_MAX && (
                <p
                  role="alert"
                  className="rounded-md bg-amber-500/15 p-3 text-sm"
                >
                  繰り返しを含む生成見込みは10,000件までです。選択を減らしてください。
                </p>
              )}
              {!!preview.skipped.length && (
                <div className="rounded-md border p-3 text-sm">
                  <p className="font-medium">取り込めない予定</p>
                  <ul className="mt-1 list-inside list-disc text-muted-foreground">
                    {preview.skipped.map((item) => (
                      <li key={item.reason}>
                        {SKIP_LABELS[item.reason]}: {item.count}件
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              <div className="rounded-lg border">
                <Label className="min-h-11 border-b px-3">
                  <Checkbox
                    checked={
                      !!preview.items.length &&
                      selectedItems.length === preview.items.length
                    }
                    onCheckedChange={(checked) =>
                      setSelected(
                        checked
                          ? new Set(
                              preview.items.map((item) => item.source_index),
                            )
                          : new Set(),
                      )
                    }
                  />
                  すべて選択
                </Label>
                <ul aria-label="取り込む予定" className="divide-y">
                  {preview.items.map((item) => {
                    const recurrence = recurrenceLabel(
                      item.event.recurrence_rule,
                    );
                    return (
                      <li key={item.source_index}>
                        <Label className="min-h-14 items-start px-3 py-2.5 leading-normal">
                          <Checkbox
                            className="mt-0.5"
                            checked={selected.has(item.source_index)}
                            onCheckedChange={(checked) =>
                              setSelected((current) => {
                                const next = new Set(current);
                                if (checked) next.add(item.source_index);
                                else next.delete(item.source_index);
                                return next;
                              })
                            }
                          />
                          <span className="min-w-0 flex-1">
                            <span className="block break-words font-medium">
                              {item.event.name}
                            </span>
                            <span className="block text-xs text-muted-foreground">
                              {formatDateTime(
                                item.event.start_at,
                                item.event.is_all_day,
                              )}
                              {" 〜 "}
                              {formatDateTime(
                                item.event.end_at,
                                item.event.is_all_day,
                              )}
                              {recurrence ? `・${recurrence}` : ""}
                            </span>
                            <span className="mt-1 flex flex-wrap gap-1">
                              {item.duplicate && (
                                <span className="rounded bg-amber-500/15 px-1.5 py-0.5 text-xs">
                                  すでにあります
                                </span>
                              )}
                              {item.truncated_fields.map((field) => (
                                <span
                                  key={field}
                                  className="rounded bg-muted px-1.5 py-0.5 text-xs"
                                >
                                  {field === "name" ? "タイトル" : "説明"}
                                  を切り詰めました
                                </span>
                              ))}
                            </span>
                          </span>
                        </Label>
                      </li>
                    );
                  })}
                </ul>
              </div>
            </>
          )}

          {loading && (
            <p role="status" className="text-sm text-muted-foreground">
              ファイルを確認しています…
            </p>
          )}
          {error && (
            <p
              role="alert"
              className="rounded-md bg-destructive/10 p-3 text-destructive"
            >
              {error}
            </p>
          )}
          {importedCount !== null && (
            <p role="status" className="rounded-md bg-emerald-500/15 p-3">
              {importedCount}件の予定を取り込みました。
            </p>
          )}
        </div>

        <DialogFooter className="m-0">
          <DialogClose render={<Button variant="outline" />}>
            閉じる
          </DialogClose>
          <Button
            type="button"
            disabled={blocked}
            onClick={() => void submit()}
          >
            {importEvents.isPending ? (
              "取り込み中…"
            ) : (
              <>
                <CalendarPlusIcon />
                {selectedItems.length}件を取り込む
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
