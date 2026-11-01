import "server-only";

const API_URL = process.env.API_URL ?? "http://127.0.0.1:8080";
/** 記録が遅れても招待の流れには関係ないので、待ち過ぎない */
const TIMEOUT_MS = 3000;
/** api が知らない値は other にまとめるので、長い値をそのまま送る必要はない */
const MAX_SOURCE_LENGTH = 64;

/**
 * 招待リンクのクリックを導入元ごとに 1 件数える (#374)。`/invite` のリダイレクトの後 (`after`) に呼ぶ。
 * 導入元の値の検証は api (POST /invite-clicks) が行い、知らない値は other として数える。
 * 記録に失敗しても利用者には関係ないので、例外は投げずにログだけ残す
 */
export async function recordInviteClick(source: string | null): Promise<void> {
  try {
    const res = await fetch(`${API_URL}/invite-clicks`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        source: source?.slice(0, MAX_SOURCE_LENGTH) ?? null,
      }),
      cache: "no-store",
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
    if (!res.ok) {
      console.warn(
        `招待リンクのクリックを記録できませんでした (${res.status})`,
      );
    }
  } catch (error) {
    console.warn("招待リンクのクリックを記録できませんでした", error);
  }
}
