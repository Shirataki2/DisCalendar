"use client";

import { addDays, format } from "date-fns";
import { useState } from "react";
import { DatePicker } from "@/components/form/date-picker";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { describeApiError } from "@/lib/api";
import type { PollDetail, PollInput, PollOptionInput } from "@/lib/api/types";
import { nowInJst, parseApiDateTime } from "@/lib/calendar-events";

export function PollFormDialog({
  poll,
  onClose,
  onSave,
}: {
  poll: PollDetail | null;
  onClose: () => void;
  onSave: (input: PollInput) => Promise<void>;
}) {
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
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>
            {poll ? "日程調整を編集" : "日程調整を作成"}
          </DialogTitle>
          <DialogDescription>
            候補を最大5件追加できます。日時は日本時間です。
            {poll && "日時を変更・削除した候補の回答はリセットされます。"}
          </DialogDescription>
        </DialogHeader>
        <form
          className="space-y-5"
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
            if (
              deadline &&
              `${deadline}:00` <= format(nowInJst(), "yyyy-MM-dd'T'HH:mm:ss")
            ) {
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
                expected_version: poll?.version,
              });
            } catch (err) {
              setError(describeApiError(err));
            } finally {
              setBusy(false);
            }
          }}
        >
          <fieldset disabled={busy} className="space-y-4">
            <label className="block space-y-1" htmlFor="poll-title">
              <span>タイトル</span>
              <Input
                id="poll-title"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                required
              />
            </label>
            <label className="block space-y-1" htmlFor="poll-description">
              <span>説明（任意）</span>
              <Textarea
                id="poll-description"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </label>
            {options.map((option, index) => (
              <fieldset
                key={option.id ?? `new-${index}`}
                className="space-y-3 rounded-lg border p-3"
              >
                <legend className="px-1 font-medium">候補 {index + 1}</legend>
                <label className="flex min-h-11 items-center gap-2">
                  <input
                    type="checkbox"
                    checked={option.is_all_day}
                    onChange={(e) =>
                      change(index, {
                        is_all_day: e.target.checked,
                        start_at: `${option.start_at.slice(0, 10)}T${e.target.checked ? "00:00:00" : "20:00:00"}`,
                        end_at: `${option.end_at.slice(0, 10)}T${e.target.checked ? "00:00:00" : "21:00:00"}`,
                      })
                    }
                  />
                  終日
                </label>
                <div className="grid gap-3 sm:grid-cols-2">
                  {(["start_at", "end_at"] as const).map((key) => (
                    <div key={key} className="min-w-0 space-y-2">
                      <label htmlFor={`poll-${index}-${key}`}>
                        {key === "start_at" ? "開始日" : "終了日"}
                      </label>
                      <DatePicker
                        id={`poll-${index}-${key}`}
                        value={parseApiDateTime(option[key])}
                        onChange={(date) =>
                          change(index, {
                            [key]: `${format(date, "yyyy-MM-dd")}${option[key].slice(10)}`,
                          })
                        }
                      />
                      {!option.is_all_day && (
                        <Input
                          aria-label={`候補${index + 1}の${key === "start_at" ? "開始" : "終了"}時刻`}
                          type="time"
                          required
                          value={option[key].slice(11, 16)}
                          onChange={(e) =>
                            change(index, {
                              [key]: `${option[key].slice(0, 10)}T${e.target.value}:00`,
                            })
                          }
                        />
                      )}
                    </div>
                  ))}
                </div>
                <Button
                  type="button"
                  variant="outline"
                  className="min-h-11"
                  disabled={options.length === 1}
                  aria-label={`候補${index + 1}を削除`}
                  onClick={() => {
                    setDirty(true);
                    setOptions(options.filter((_, i) => i !== index));
                  }}
                >
                  候補を削除
                </Button>
              </fieldset>
            ))}
            <Button
              type="button"
              variant="outline"
              className="min-h-11"
              disabled={options.length >= 5}
              onClick={() => {
                setDirty(true);
                setOptions([...options, newOption(options.length + 1)]);
              }}
            >
              候補を追加
            </Button>
            <label className="block space-y-1" htmlFor="poll-deadline">
              <span>締切（任意・日本時間）</span>
              <Input
                id="poll-deadline"
                type="datetime-local"
                value={deadline}
                onChange={(e) => setDeadline(e.target.value)}
              />
            </label>
          </fieldset>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              variant="outline"
              className="min-h-11"
              disabled={busy}
              onClick={close}
            >
              キャンセル
            </Button>
            <Button type="submit" className="min-h-11" disabled={busy}>
              {busy ? "保存中…" : poll ? "変更を保存" : "日程調整を作成"}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
function newOption(days = 1): PollOptionInput {
  const date = format(addDays(nowInJst(), days), "yyyy-MM-dd");
  return {
    start_at: `${date}T20:00:00`,
    end_at: `${date}T21:00:00`,
    is_all_day: false,
  };
}
