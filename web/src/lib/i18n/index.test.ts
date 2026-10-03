import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";
import { ApiError, describeApiError } from "@/lib/api/client";
import {
  describeEventRange,
  describeNotification,
} from "@/lib/calendar-events";
import {
  defaultEventFormValues,
  eventFormSchema,
  eventFormToApiInput,
  NOTIFICATION_UNITS,
} from "@/lib/event-form";
import { JAPANESE_HOLIDAYS } from "@/lib/japanese-holidays.generated";
import { describeRecurrence } from "@/lib/recurrence";
import { formatDisplayDate, resolveLanguage, translate } from "./index";

describe("表示言語", () => {
  test("地域付き英語を選び、未対応・不正な値は日本語にする", () => {
    for (const value of ["en", "en-US", "EN-gb"])
      expect(resolveLanguage(value)).toBe("en");
    for (const value of ["ja-JP", "fr", "english", "", null, {}])
      expect(resolveLanguage(value)).toBe("ja");
  });
  test("未定義の英訳は日本語原文を表示し、入力値は翻訳や再展開しない", () => {
    expect(translate("en", "未翻訳の案内")).toBe("未翻訳の案内");
    expect(
      translate("en", "「{name}」を削除します。この操作は取り消せません。", {
        name: "予定 {name} <b>",
      }),
    ).toBe("“予定 {name} <b>” will be deleted. This cannot be undone.");
    expect(translate("en", "toString")).toBe("toString");
  });
  test("同じ検証ルールのエラーを英語で表示する", () => {
    const values = defaultEventFormValues(new Date("2026-10-03T03:00:00Z"));
    const result = eventFormSchema.safeParse(values);
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(translate("en", result.error.issues[0].message)).toBe(
        "Enter a title",
      );
    }
    expect(describeApiError(new ApiError(403, "forbidden", "raw"), "en")).toBe(
      "You do not have permission to do this",
    );
    expect(
      describeApiError(
        new ApiError(400, "bad_request", "name is too long"),
        "en",
      ),
    ).toBe("Invalid input (name is too long)");
    const overlap =
      "過去の開催枠と重なるため、この回以降をこの日へ移動できません。別の開始日か「この回のみ」を選択してください";
    const recurringError = new ApiError(400, "bad_request", overlap);
    expect(describeApiError(recurringError, "en")).toBe(
      "Invalid input (This move overlaps earlier occurrences. Choose another start date or “This occurrence”)",
    );
    expect(describeApiError(recurringError)).toBe(
      `入力内容が正しくありません (${overlap})`,
    );
  });
  test("添付 API の検証・保存先エラーの詳細をすべて英訳する", () => {
    for (const path of [
      "attachments.rs",
      "models/attachments.rs",
      "routes/attachments.rs",
    ]) {
      const source = readFileSync(
        new URL(`../../../../api/src/${path}`, import.meta.url),
        "utf8",
      );
      const messages = [
        ...source.matchAll(/ApiError::(BadRequest|Unavailable)\(\s*"([^"]+)"/g),
      ];
      expect(messages.length).toBeGreaterThan(0);
      for (const [, kind, message] of messages) {
        const error = new ApiError(
          400,
          kind === "BadRequest" ? "bad_request" : "unavailable",
          message,
        );
        expect(describeApiError(error, "en"), message).not.toMatch(
          /[ぁ-んァ-ン一-龯]/,
        );
        expect(describeApiError(error, "ja")).toContain(message);
      }
    }
  });
  test("サーバー件数に合わせて英語の単数・複数形を選ぶ", () => {
    for (const message of [
      "{count} サーバーの予定をまとめて表示",
      "{count} サーバーの予定をまとめて表示しています。予定の作成・編集は各サーバーのカレンダーで行えます",
      "他 {count} サーバーを表示",
    ]) {
      for (const count of [0, 1, 2]) {
        expect(translate("en", message, { count })).toContain(
          `${count} ${message.startsWith("他") ? "more " : ""}server${count === 1 ? "" : "s"}`,
        );
        expect(translate("en", message, { count })).not.toContain(
          count === 1 ? "servers" : "{count}",
        );
        expect(translate("ja", message, { count })).toBe(
          message.replace("{count}", String(count)),
        );
      }
    }
  });
  test("同梱した祝日名は過去の名称を含めてすべて英訳する", () => {
    for (const name of new Set(Object.values(JAPANESE_HOLIDAYS))) {
      expect(translate("en", name), name).not.toBe(name);
      expect(translate("ja", name)).toBe(name);
    }
    expect(translate("en", JAPANESE_HOLIDAYS["2019-10-14"])).toBe(
      "Health and Sports Day (Sports Day)",
    );
  });
  test("通知単位の単数・複数形を予定詳細の表記に合わせる", () => {
    for (const { value, label } of NOTIFICATION_UNITS) {
      for (const num of [1, 2]) {
        expect(`${num} ${translate("en", label, { count: num })}`).toBe(
          describeNotification({ num, unit: value }, "en"),
        );
        expect(translate("ja", label, { count: num })).toBe(label);
      }
    }
  });
  test("繰り返しの終了日が空・不正でも英語の説明を表示できる", () => {
    for (const date of ["", "invalid", "2026-02-30"]) {
      expect(
        describeRecurrence(
          { frequency: "daily", end: { type: "until", date } },
          "en",
        ),
      ).toBe("Daily / select a valid end date");
    }
    expect(
      describeRecurrence(
        { frequency: "daily", end: { type: "until", date: "2026-10-03" } },
        "en",
      ),
    ).toBe("Daily / until Oct 3, 2026");
  });
  test("言語に合わせた日付・通知の表記でも JST と終日範囲・送信値を変えない", () => {
    const values = {
      ...defaultEventFormValues(new Date("2026-10-03T03:00:00Z")),
      name: "日本語の予定",
    };
    const input = eventFormToApiInput(values);
    const before = JSON.stringify(input);
    expect(
      describeEventRange(
        {
          start_at: "2026-10-03T00:00:00",
          end_at: "2026-10-04T00:00:00",
          is_all_day: true,
        },
        "en",
      ),
    ).toBe("Oct 3, 2026 – Oct 4, 2026");
    expect(
      formatDisplayDate(new Date(2026, 9, 3, 13), "en", {
        hour: "2-digit",
        minute: "2-digit",
        hourCycle: "h23",
      }),
    ).toContain("13:00");
    expect(describeNotification({ num: 1, unit: "days" }, "en")).toBe(
      "1 day before",
    );
    expect(describeNotification({ num: 2, unit: "hours" }, "en")).toBe(
      "2 hours before",
    );
    expect(describeNotification({ num: 1, unit: "days" })).toBe("1日前");
    expect(
      describeRecurrence(
        {
          frequency: "weekly",
          weekdays: [0, 4],
          end: { type: "count", count: 2 },
        },
        "en",
      ),
    ).toBe("Weekly on Mon, Fri / 2 occurrences");
    expect(JSON.stringify(input)).toBe(before);
    expect(input.name).toBe("日本語の予定");
  });
});
