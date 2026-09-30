"use client";

import { addDays, format } from "date-fns";
import { PlusIcon, XIcon } from "lucide-react";
import { type CSSProperties, Fragment, useState } from "react";
import { DatePicker } from "@/components/form/date-picker";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { describeApiError } from "@/lib/api";
import type { PollDetail, PollInput, PollOptionInput } from "@/lib/api/types";
import { nowInJst, parseApiDateTime } from "@/lib/calendar-events";

const MAX_OPTIONS = 5;
const optionGridStyle = {
  "--poll-option-cols":
    "1.5rem minmax(0,1fr) 7.5rem 1rem minmax(0,1fr) 7.5rem 2.5rem 2.25rem",
} as CSSProperties;

export function PollFormDialog({
  poll,
  onClose,
  onSave,
}: {
  poll: PollDetail | null;
  onClose: () => void;
  onSave: (input: PollInput) => Promise<void>;
}) {
  const [version] = useState(poll?.version);
  const [title, setTitle] = useState(poll?.title ?? "");
  const [description, setDescription] = useState(poll?.description ?? "");
  const [deadline, setDeadline] = useState(poll?.deadline?.slice(0, 16) ?? "");
  const [options, setOptions] = useState<PollOptionInput[]>(
    poll?.options ?? [newOption()],
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [dirty, setDirty] = useState(false);
  const close = () => {
    if (!busy && (!dirty || window.confirm("入力中の変更を破棄しますか？")))
      onClose();
  };
  function change(index: number, update: Partial<PollOptionInput>) {
    setDirty(true);
    setOptions(options.map((o, i) => (i === index ? { ...o, ...update } : o)));
  }
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) close();
      }}
      disablePointerDismissal
    >
      <DialogContent className="flex max-h-[calc(100dvh-2rem)] flex-col gap-0 overflow-hidden p-0 sm:max-w-4xl [&>[data-slot=dialog-close]]:size-11">
        <DialogHeader className="shrink-0 border-b p-4 pr-14">
          <DialogTitle>
            {poll ? "日程調整を編集" : "日程調整を作成"}
          </DialogTitle>
          <DialogDescription>
            候補を最大{MAX_OPTIONS}件追加できます。日時は日本時間です。
            {poll && "日時を変更・削除した候補の回答はリセットされます。"}
          </DialogDescription>
        </DialogHeader>
        <form
          className="flex min-h-0 flex-col"
          onChange={() => setDirty(true)}
          onSubmit={async (e) => {
            e.preventDefault();
            setError("");
            if (
              !title.trim() ||
              [...title].length > 32 ||
              [...description].length > 1000
            ) {
              setError(
                "タイトルは1〜32文字、説明は1000文字以内で入力してください",
              );
              return;
            }
            if (deadline && Date.parse(`${deadline}:00+09:00`) <= Date.now()) {
              setError("締切は未来の日時にしてください");
              return;
            }
            if (
              options.some(
                (o) =>
                  o.end_at < o.start_at ||
                  (!o.is_all_day && o.end_at === o.start_at),
              ) ||
              new Set(
                options.map((o) => `${o.start_at}/${o.end_at}/${o.is_all_day}`),
              ).size !== options.length
            ) {
              setError("候補の開始・終了日時と重複を確認してください");
              return;
            }
            setBusy(true);
            try {
              await onSave({
                title,
                description: description || null,
                deadline: deadline ? `${deadline}:00` : null,
                options,
                expected_version: version,
              });
            } catch (err) {
              setError(describeApiError(err));
            } finally {
              setBusy(false);
            }
          }}
        >
          <div className="min-h-0 overflow-y-auto overscroll-contain p-4">
            <fieldset disabled={busy} className="min-w-0 space-y-4">
              <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_14rem]">
                <label className="block space-y-1 text-sm" htmlFor="poll-title">
                  <span className="font-medium">タイトル</span>
                  <Input
                    id="poll-title"
                    value={title}
                    onChange={(e) => setTitle(e.target.value)}
                    required
                  />
                </label>
                <label
                  className="block space-y-1 text-sm"
                  htmlFor="poll-deadline"
                >
                  <span className="font-medium">締切（任意・日本時間）</span>
                  <Input
                    id="poll-deadline"
                    type="datetime-local"
                    value={deadline}
                    onChange={(e) => setDeadline(e.target.value)}
                  />
                </label>
              </div>
              <label
                className="block space-y-1 text-sm"
                htmlFor="poll-description"
              >
                <span className="font-medium">説明（任意）</span>
                <Textarea
                  id="poll-description"
                  rows={2}
                  className="min-h-14"
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                />
              </label>
              <section
                aria-labelledby="poll-options-heading"
                className="space-y-2"
              >
                <div className="flex items-center justify-between gap-2">
                  <h3 id="poll-options-heading" className="text-sm font-medium">
                    候補
                  </h3>
                  <span className="text-xs text-muted-foreground">
                    {options.length} / {MAX_OPTIONS}
                  </span>
                </div>
                {/* PC 幅では 1 候補を 1 行 (番号・開始・終了・終日・削除) に収める */}
                <div
                  aria-hidden
                  className="hidden gap-2 px-2 text-xs text-muted-foreground sm:grid sm:grid-cols-(--poll-option-cols)"
                  style={optionGridStyle}
                >
                  <span />
                  <span>開始日</span>
                  <span>時刻</span>
                  <span />
                  <span>終了日</span>
                  <span>時刻</span>
                  <span>終日</span>
                  <span />
                </div>
                <ol className="space-y-2">
                  {options.map((option, index) => (
                    <li
                      key={option.id ?? `new-${index}`}
                      aria-label={`候補 ${index + 1}`}
                      className="grid grid-cols-[1.5rem_minmax(0,1fr)_7.5rem] items-center gap-2 rounded-md border px-2 py-1.5 sm:grid-cols-(--poll-option-cols)"
                      style={optionGridStyle}
                    >
                      <span className="flex size-6 items-center justify-center rounded-full bg-muted text-xs font-medium">
                        {index + 1}
                      </span>
                      {(["start_at", "end_at"] as const).map((key) => {
                        const label = key === "start_at" ? "開始" : "終了";
                        return (
                          <Fragment key={key}>
                            {key === "end_at" && (
                              <span
                                aria-hidden
                                className="text-center text-muted-foreground"
                              >
                                〜
                              </span>
                            )}
                            <div className="min-w-0">
                              <label
                                className="sr-only"
                                htmlFor={`poll-${index}-${key}`}
                              >
                                {`候補${index + 1}の${label}日`}
                              </label>
                              <DatePicker
                                id={`poll-${index}-${key}`}
                                className="h-9 px-2"
                                value={parseApiDateTime(option[key])}
                                onChange={(date) =>
                                  change(index, {
                                    [key]: `${format(date, "yyyy-MM-dd")}${option[key].slice(10)}`,
                                  })
                                }
                              />
                            </div>
                            {option.is_all_day ? (
                              <span className="text-center text-xs text-muted-foreground">
                                終日
                              </span>
                            ) : (
                              <Input
                                aria-label={`候補${index + 1}の${label}時刻`}
                                type="time"
                                required
                                className="h-9"
                                value={option[key].slice(11, 16)}
                                onChange={(e) =>
                                  change(index, {
                                    [key]: `${option[key].slice(0, 10)}T${e.target.value}:00`,
                                  })
                                }
                              />
                            )}
                          </Fragment>
                        );
                      })}
                      <label className="col-span-2 flex min-h-9 items-center gap-2 text-sm sm:col-span-1 sm:justify-center">
                        <input
                          type="checkbox"
                          className="size-4"
                          checked={option.is_all_day}
                          aria-label={`候補${index + 1}を終日にする`}
                          onChange={(e) =>
                            change(index, {
                              is_all_day: e.target.checked,
                              start_at: `${option.start_at.slice(0, 10)}T${e.target.checked ? "00:00:00" : "20:00:00"}`,
                              end_at: `${option.end_at.slice(0, 10)}T${e.target.checked ? "00:00:00" : "21:00:00"}`,
                            })
                          }
                        />
                        <span className="sm:hidden">終日</span>
                      </label>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon-lg"
                        className="justify-self-end"
                        disabled={options.length === 1}
                        aria-label={`候補${index + 1}を削除`}
                        onClick={() => {
                          setDirty(true);
                          setOptions(options.filter((_, i) => i !== index));
                        }}
                      >
                        <XIcon />
                      </Button>
                    </li>
                  ))}
                </ol>
                <Button
                  type="button"
                  variant="outline"
                  className="min-h-11 w-full border-dashed"
                  disabled={options.length >= MAX_OPTIONS}
                  onClick={() => {
                    setDirty(true);
                    setOptions([...options, nextOption(options.at(-1))]);
                  }}
                >
                  <PlusIcon />
                  候補を追加
                </Button>
              </section>
            </fieldset>
          </div>
          {error && (
            <p
              role="alert"
              className="shrink-0 bg-destructive/10 px-4 py-2 text-sm text-destructive"
            >
              {error}
            </p>
          )}
          <DialogFooter className="m-0 shrink-0 flex-row justify-end [&>button]:min-h-11">
            <Button
              type="button"
              variant="outline"
              disabled={busy}
              onClick={close}
            >
              キャンセル
            </Button>
            <Button type="submit" disabled={busy}>
              {busy ? "保存中…" : poll ? "変更を保存" : "日程調整を作成"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
/** 直前の候補の翌日・同じ時間帯を次の候補にする (連日の候補を続けて入れやすくする) */
function nextOption(last: PollOptionInput | undefined): PollOptionInput {
  if (!last) return newOption();
  const shift = (value: string) =>
    `${format(addDays(parseApiDateTime(value), 1), "yyyy-MM-dd")}${value.slice(10)}`;
  return {
    start_at: shift(last.start_at),
    end_at: shift(last.end_at),
    is_all_day: last.is_all_day,
  };
}
function newOption(days = 1): PollOptionInput {
  const date = format(addDays(nowInJst(), days), "yyyy-MM-dd");
  return {
    start_at: `${date}T20:00:00`,
    end_at: `${date}T21:00:00`,
    is_all_day: false,
  };
}
