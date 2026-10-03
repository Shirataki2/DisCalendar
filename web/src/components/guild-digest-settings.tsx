"use client";

import { type KeyboardEvent, useState } from "react";
import { useLanguage } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { describeApiError } from "@/lib/api";
import type { GuildDigestSettings } from "@/lib/api/types";
import { useGuildDigestQuery, useUpdateGuildDigest } from "@/lib/query/guild";

const DAYS = ["月曜", "火曜", "水曜", "木曜", "金曜", "土曜", "日曜"];

export function GuildDigestSettingsSection({
  guildId,
  canManage,
  configured,
}: {
  guildId: string;
  canManage: boolean;
  configured: boolean | undefined;
}) {
  const { t, language } = useLanguage();
  const query = useGuildDigestQuery(guildId);
  const update = useUpdateGuildDigest(guildId);
  const [draft, setDraft] = useState<GuildDigestSettings | null>(null);
  const [error, setError] = useState<string | Error | null>(null);
  const [saved, setSaved] = useState(false);
  const values = draft ?? query.data;
  const disabled =
    !canManage || update.isPending || query.isFetching || query.isError;
  const canSave =
    !!values &&
    !disabled &&
    (!!configured || (!values.daily_enabled && !values.weekly_enabled));
  const change = (patch: Partial<GuildDigestSettings>) => {
    if (!values) return;
    setDraft({ ...values, ...patch });
    setSaved(false);
    setError(null);
  };
  const save = async () => {
    if (!values) return;
    setError(null);
    if (
      ![values.daily_time, values.weekly_time].every((time) =>
        /^([01]\d|2[0-3]):[0-5]\d$/.test(time),
      )
    ) {
      setError("投稿時刻を 00:00〜23:59 で入力してください");
      return;
    }
    try {
      await update.mutateAsync(values);
      setDraft(null);
      setSaved(true);
    } catch (cause) {
      setError(cause instanceof Error ? cause : new Error());
    }
  };
  const saveOnEnter = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter" && !event.nativeEvent.isComposing) {
      event.preventDefault();
      if (canSave) void save();
    }
  };
  return (
    <section aria-labelledby="guild-digest-heading" className="grid gap-4">
      <h3 id="guild-digest-heading" className="text-sm font-medium">
        {t("予定のまとめを投稿する")}
      </h3>
      <p className="text-sm text-muted-foreground">
        {t(
          "通知先チャンネルへ投稿します。時刻は日本時間 (JST) です。この節は「まとめ投稿を保存」で反映します。",
        )}
      </p>
      {configured === false && (
        <p className="text-sm text-muted-foreground">
          {t(
            "先に通知先を設定してください。上の「通知先チャンネル」を選んで「保存」するか、Discord で /init を実行します。",
          )}
        </p>
      )}
      {query.isError ? (
        <p role="alert" className="text-sm text-destructive">
          {t(
            "まとめ投稿の設定を取得できませんでした。ダイアログを開き直してください。",
          )}
        </p>
      ) : !values ? (
        <p role="status" className="text-sm">
          {t("まとめ投稿の設定を読み込み中…")}
        </p>
      ) : (
        <fieldset disabled={disabled} className="grid min-w-0 gap-4">
          <legend className="sr-only">{t("まとめ投稿の設定")}</legend>
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="grid content-start gap-2">
              <label
                className="flex min-h-11 items-center gap-2 text-sm"
                htmlFor="digest-daily-enabled"
              >
                <Checkbox
                  id="digest-daily-enabled"
                  checked={values.daily_enabled}
                  disabled={disabled || (!configured && !values.daily_enabled)}
                  onCheckedChange={(checked) =>
                    change({ daily_enabled: checked === true })
                  }
                />
                {t("毎日、今日の予定を投稿する")}
              </label>
              <label htmlFor="digest-daily-time" className="text-sm">
                {t("毎日の投稿時刻 (JST)")}
              </label>
              <Input
                id="digest-daily-time"
                type="time"
                step={60}
                onKeyDown={saveOnEnter}
                value={values.daily_time}
                disabled={disabled || !values.daily_enabled}
                onChange={(e) => change({ daily_time: e.target.value })}
                className="min-h-11"
              />
            </div>
            <div className="grid content-start gap-2">
              <label
                className="flex min-h-11 items-center gap-2 text-sm"
                htmlFor="digest-weekly-enabled"
              >
                <Checkbox
                  id="digest-weekly-enabled"
                  checked={values.weekly_enabled}
                  disabled={disabled || (!configured && !values.weekly_enabled)}
                  onCheckedChange={(checked) =>
                    change({ weekly_enabled: checked === true })
                  }
                />
                {t("毎週、今週の予定を投稿する")}
              </label>
              <label htmlFor="digest-weekly-day" className="text-sm">
                {t("毎週の投稿曜日")}
              </label>
              <select
                id="digest-weekly-day"
                value={values.weekly_day}
                disabled={disabled || !values.weekly_enabled}
                onChange={(e) => change({ weekly_day: Number(e.target.value) })}
                className="min-h-11 w-full rounded-md border border-input bg-background px-3 text-sm focus-visible:outline-2 focus-visible:outline-ring"
              >
                {DAYS.map((day, index) => (
                  <option key={day} value={index}>
                    {t(day)}
                  </option>
                ))}
              </select>
              <label htmlFor="digest-weekly-time" className="text-sm">
                {t("毎週の投稿時刻 (JST)")}
              </label>
              <Input
                id="digest-weekly-time"
                type="time"
                step={60}
                onKeyDown={saveOnEnter}
                value={values.weekly_time}
                disabled={disabled || !values.weekly_enabled}
                onChange={(e) => change({ weekly_time: e.target.value })}
                className="min-h-11"
              />
              <p className="text-sm text-muted-foreground">
                {t("投稿する曜日から7日間の予定をまとめます。")}
              </p>
            </div>
          </div>
          <label
            className="flex min-h-11 items-center gap-2 text-sm"
            htmlFor="digest-skip-empty"
          >
            <Checkbox
              id="digest-skip-empty"
              checked={values.skip_empty}
              disabled={disabled}
              onCheckedChange={(checked) =>
                change({ skip_empty: checked === true })
              }
            />
            {t("予定が無い日は投稿しない")}
          </label>
          <p className="text-sm text-muted-foreground">
            {t(
              "毎週の投稿では、7日間に予定が無いときに省略します。外すと「予定はありません」と投稿します。",
            )}
          </p>
          <Button
            type="button"
            variant="outline"
            className="min-h-11 justify-self-start"
            disabled={!canSave}
            onClick={save}
          >
            {update.isPending
              ? t("まとめ投稿を保存中…")
              : t("まとめ投稿を保存")}
          </Button>
        </fieldset>
      )}
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {typeof error === "string"
            ? t(error)
            : describeApiError(error, language)}
        </p>
      )}
      {saved && (
        <p role="status" className="text-sm">
          {t("まとめ投稿の設定を保存しました")}
        </p>
      )}
    </section>
  );
}
