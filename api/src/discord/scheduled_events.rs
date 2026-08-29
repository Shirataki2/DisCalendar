//! Guild Scheduled Events API の呼び出し (#94)。
//!
//! 「Discord のイベントとしても作成する」を有効にした予定を、Bot トークンで
//! Discord のスケジュールイベントとして作成・変更・削除する。
//! チャンネルに紐付けない外部イベント (EXTERNAL) として作り、場所には
//! ギルドのダッシュボード URL を入れる。
//! <https://discord.com/developers/docs/resources/guild-scheduled-event>

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use super::{DiscordClient, DiscordError, checked_id};

/// チャンネルに紐付けない外部イベント。`scheduled_end_time` と `entity_metadata.location` が必須
const ENTITY_TYPE_EXTERNAL: u8 = 3;
/// GUILD_ONLY。Discord は現状これ以外の値を受け付けない
const PRIVACY_LEVEL_GUILD_ONLY: u8 = 2;

/// スケジュールイベントの作成・変更リクエストのボディ。
/// 変更 (PATCH) でも全フィールドを送る (説明を消したいときは `description: null` を送る必要があるため、
/// `None` でもフィールドを省略しない)
#[derive(Debug, PartialEq, Serialize)]
pub struct ScheduledEventPayload {
    name: String,
    description: Option<String>,
    scheduled_start_time: String,
    scheduled_end_time: String,
    privacy_level: u8,
    entity_type: u8,
    entity_metadata: EntityMetadata,
}

#[derive(Debug, PartialEq, Serialize)]
struct EntityMetadata {
    location: String,
}

impl ScheduledEventPayload {
    /// 予定の値から Discord へ送るボディを組み立てる。
    /// `site_base_url` は web の公開 URL (末尾スラッシュなし。staging では staging のドメイン)。
    ///
    /// 日時はアプリの慣習 (タイムゾーンなしの JST) から `+09:00` 付きの ISO 8601 に変換する。
    /// 終日予定は DB 上「`start_at` = 開始日 0:00、`end_at` = 終了日 (期間に含む) の 0:00」なので、
    /// Discord の終了時刻 (排他的) には `end_at` の翌日 0:00 を渡す
    /// (翌日が無い終了日は `validate_discord_flag` が事前に弾いている)
    pub fn new(
        site_base_url: &str,
        guild_id: &str,
        name: &str,
        description: Option<&str>,
        is_all_day: bool,
        start_at: NaiveDateTime,
        end_at: NaiveDateTime,
    ) -> Self {
        let (start, end) = if is_all_day {
            let midnight = |d: chrono::NaiveDate| d.and_hms_opt(0, 0, 0).expect("valid time");
            (
                midnight(start_at.date()),
                midnight(
                    end_at
                        .date()
                        .succ_opt()
                        .expect("checked by validate_discord_flag"),
                ),
            )
        } else {
            (start_at, end_at)
        };
        Self {
            name: name.to_owned(),
            // Discord は説明を送るなら 1 文字以上でないと 400 にする。予定側は空の説明を
            // 許しているので (web は空欄を null にするが、API を直接叩けば空文字が来る)、
            // 中身が無ければ「説明なし」に揃える
            description: description
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(str::to_owned),
            scheduled_start_time: to_iso8601_jst(start),
            scheduled_end_time: to_iso8601_jst(end),
            privacy_level: PRIVACY_LEVEL_GUILD_ONLY,
            entity_type: ENTITY_TYPE_EXTERNAL,
            entity_metadata: EntityMetadata {
                // 「場所」にはギルドのカレンダーの URL を入れる (Discord 側の表示から予定に辿れるように)
                location: format!("{site_base_url}/dashboard/{guild_id}"),
            },
        }
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("payload is serializable")
    }
}

/// タイムゾーンなしの JST を `+09:00` 付きの ISO 8601 にする
fn to_iso8601_jst(t: NaiveDateTime) -> String {
    format!("{}+09:00", t.format("%Y-%m-%dT%H:%M:%S"))
}

/// 作成・変更のレスポンスのうち使う部分
#[derive(Deserialize)]
struct ScheduledEventResponse {
    id: String,
}

impl DiscordClient {
    /// スケジュールイベントを作成し、その ID を返す
    pub async fn create_scheduled_event(
        &self,
        guild_id: &str,
        payload: &ScheduledEventPayload,
    ) -> Result<String, DiscordError> {
        let sent = self
            .send(
                reqwest::Method::POST,
                &format!("/guilds/{}/scheduled-events", checked_id(guild_id)?),
                Some(&payload.to_json()),
            )
            .await;
        self.invalidate_on_forbidden(guild_id, &sent).await;
        let Some(text) = sent? else {
            // 作成先のギルドが無い = Bot が退出・追放されている (#122)。
            // 権限のキャッシュが「参加中」のままだと同じ失敗を繰り返すので捨てておく
            // (次の取得で Bot 未参加になり、チェックボックスは案内つきで無効になる)
            self.invalidate_guild_permissions(guild_id).await;
            return Err(DiscordError::GuildGone);
        };
        let created: ScheduledEventResponse = serde_json::from_str(&text)
            .map_err(|_| DiscordError::Unexpected("unexpected scheduled event response"))?;
        Ok(created.id)
    }

    /// スケジュールイベントを変更する。Discord 側で既に削除されていたら `Ok(false)`
    pub async fn modify_scheduled_event(
        &self,
        guild_id: &str,
        scheduled_event_id: &str,
        payload: &ScheduledEventPayload,
    ) -> Result<bool, DiscordError> {
        let sent = self
            .send(
                reqwest::Method::PATCH,
                &format!(
                    "/guilds/{}/scheduled-events/{}",
                    checked_id(guild_id)?,
                    checked_id(scheduled_event_id)?
                ),
                Some(&payload.to_json()),
            )
            .await;
        self.invalidate_on_forbidden(guild_id, &sent).await;
        Ok(sent?.is_some())
    }

    /// スケジュールイベントを削除する。Discord 側で既に削除されていたら `Ok(false)`。
    ///
    /// 404 だったときは権限キャッシュも捨てる (#122)。イベントが既に消えているだけのことも多いが、
    /// **Bot が退出してギルドごと見えない**場合も同じ 404 で返る (Discord のエラーコードは
    /// [`DiscordClient::send`] が 404 を `Ok(None)` に畳む時点で失われる)。
    /// 削除はベストエフォートでこの先の処理が無いので、ここで捨てておかないと
    /// キャッシュ上の「参加中・権限あり」が最大 5 分残り、次の連携も同じように失敗する。
    /// 捨てても余分な問い合わせは次に権限が要るときの 1 回だけで済む
    pub async fn delete_scheduled_event(
        &self,
        guild_id: &str,
        scheduled_event_id: &str,
    ) -> Result<bool, DiscordError> {
        let sent = self
            .send(
                reqwest::Method::DELETE,
                &format!(
                    "/guilds/{}/scheduled-events/{}",
                    checked_id(guild_id)?,
                    checked_id(scheduled_event_id)?
                ),
                None,
            )
            .await;
        self.invalidate_on_forbidden(guild_id, &sent).await;
        let deleted = sent?.is_some();
        if !deleted {
            self.invalidate_guild_permissions(guild_id).await;
        }
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_event_gets_jst_offset() {
        let p = ScheduledEventPayload::new(
            "https://discalendar.app",
            "123",
            "定例",
            Some("説明"),
            false,
            "2026-08-22T10:00:00".parse().unwrap(),
            "2026-08-22T11:30:00".parse().unwrap(),
        );
        assert_eq!(p.scheduled_start_time, "2026-08-22T10:00:00+09:00");
        assert_eq!(p.scheduled_end_time, "2026-08-22T11:30:00+09:00");
        assert_eq!(p.description.as_deref(), Some("説明"));
        assert_eq!(
            p.entity_metadata.location,
            "https://discalendar.app/dashboard/123"
        );
    }

    #[test]
    fn blank_description_is_sent_as_none() {
        // 予定側は空の説明を許すが、Discord は説明を送るなら 1 文字以上を要求する。
        // 空文字や空白だけの説明で 400 にならないよう「説明なし」に揃える
        for description in ["", "   ", " \n "] {
            let p = ScheduledEventPayload::new(
                "https://discalendar.app",
                "123",
                "定例",
                Some(description),
                false,
                "2026-08-22T10:00:00".parse().unwrap(),
                "2026-08-22T11:30:00".parse().unwrap(),
            );
            assert_eq!(p.description, None, "{description:?}");
        }
        // 前後の空白は落とすが、中身があればそのまま送る
        let p = ScheduledEventPayload::new(
            "https://discalendar.app",
            "123",
            "定例",
            Some("  メモ  "),
            false,
            "2026-08-22T10:00:00".parse().unwrap(),
            "2026-08-22T11:30:00".parse().unwrap(),
        );
        assert_eq!(p.description.as_deref(), Some("メモ"));
    }

    #[test]
    fn all_day_event_ends_at_next_midnight() {
        // 8/22〜8/23 の 2 日間の終日予定 (end_at は期間に含む 8/23 の 0:00)
        let p = ScheduledEventPayload::new(
            "https://discalendar.app",
            "123",
            "合宿",
            None,
            true,
            "2026-08-22T00:00:00".parse().unwrap(),
            "2026-08-23T00:00:00".parse().unwrap(),
        );
        assert_eq!(p.scheduled_start_time, "2026-08-22T00:00:00+09:00");
        // 排他的な終了時刻なので 8/24 の 0:00
        assert_eq!(p.scheduled_end_time, "2026-08-24T00:00:00+09:00");
    }

    #[test]
    fn all_day_event_ignores_stored_time_of_day() {
        // 時刻が 0:00 でない終日予定でも日付だけを見る (Bot の /create 由来の揺れへの保険)
        let p = ScheduledEventPayload::new(
            "https://discalendar.app",
            "123",
            "祭り",
            None,
            true,
            "2026-08-22T09:30:00".parse().unwrap(),
            "2026-08-22T18:00:00".parse().unwrap(),
        );
        assert_eq!(p.scheduled_start_time, "2026-08-22T00:00:00+09:00");
        assert_eq!(p.scheduled_end_time, "2026-08-23T00:00:00+09:00");
    }

    #[test]
    fn description_none_is_serialized_as_null() {
        let p = ScheduledEventPayload::new(
            "https://discalendar.app",
            "123",
            "会議",
            None,
            false,
            "2026-08-22T10:00:00".parse().unwrap(),
            "2026-08-22T11:00:00".parse().unwrap(),
        );
        let json = p.to_json();
        // PATCH で説明を消せるように、None でもフィールド自体は送る
        assert!(json.get("description").is_some_and(|v| v.is_null()));
        assert_eq!(json["privacy_level"], 2);
        assert_eq!(json["entity_type"], 3);
    }
}
