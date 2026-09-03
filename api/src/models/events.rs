use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgExecutor, PgPool};
use utoipa::ToSchema;

use super::notifications::Notification;
use crate::error::ApiError;

/// タイトルの最大文字数 (旧 Web のフォームと同じ)
pub const NAME_MAX_CHARS: usize = 32;
pub const DESCRIPTION_MAX_CHARS: usize = 1000;
pub const NOTIFICATIONS_MAX: usize = 10;

/// `events` テーブルの行
#[derive(Debug)]
pub struct EventRow {
    pub id: i32,
    pub guild_id: String,
    pub name: String,
    pub description: Option<String>,
    /// DB に入っている JSONB そのまま (`Notification::decode_all` で読む)
    pub notifications: Value,
    pub color: String,
    pub is_all_day: bool,
    pub start_at: NaiveDateTime,
    pub end_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
    /// 連携している Discord スケジュールイベントの ID (`event_discord_links`、#94)。未連携なら `None`
    pub discord_scheduled_event_id: Option<String>,
}

/// API レスポンスの予定。日時はタイムゾーンなしの JST (`YYYY-MM-DDTHH:MM:SS`)
#[derive(Debug, Serialize, ToSchema)]
pub struct Event {
    #[schema(example = 1)]
    pub id: i32,
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    #[schema(example = "定例ミーティング")]
    pub name: String,
    pub description: Option<String>,
    pub notifications: Vec<Notification>,
    #[schema(example = "#2196F3")]
    pub color: String,
    pub is_all_day: bool,
    #[schema(example = "2026-08-22T10:00:00")]
    pub start_at: NaiveDateTime,
    #[schema(example = "2026-08-22T11:00:00")]
    pub end_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
    /// 連携している Discord スケジュールイベントの ID。未連携なら `null`
    #[schema(example = "1024667289529835550")]
    pub discord_scheduled_event_id: Option<String>,
}

impl From<EventRow> for Event {
    fn from(row: EventRow) -> Self {
        Self {
            id: row.id,
            guild_id: row.guild_id,
            name: row.name,
            description: row.description,
            notifications: Notification::decode_all(&row.notifications),
            color: row.color,
            is_all_day: row.is_all_day,
            start_at: row.start_at,
            end_at: row.end_at,
            created_at: row.created_at,
            discord_scheduled_event_id: row.discord_scheduled_event_id,
        }
    }
}

/// 予定の作成・更新リクエスト
#[derive(Debug, Deserialize, ToSchema)]
pub struct EventInput {
    #[schema(example = "定例ミーティング", max_length = 32)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub notifications: Vec<Notification>,
    /// `#RRGGBB`
    #[schema(example = "#2196F3")]
    pub color: String,
    #[serde(default)]
    pub is_all_day: bool,
    /// タイムゾーンなしの JST
    #[schema(example = "2026-08-22T10:00:00")]
    pub start_at: NaiveDateTime,
    #[schema(example = "2026-08-22T11:00:00")]
    pub end_at: NaiveDateTime,
    /// Discord のスケジュールイベントとしても作成・同期するか (#94)。
    /// 通常 API (web のダイアログ) だけが見るフラグで、管理コンソールのルートでは無視される。
    /// 省略時 (`None`) は、作成では「作らない」、更新では「現在の連携状態を保持」として扱う
    /// (このフラグを知らない古いクライアントからの更新で、既存の連携が意図せず外れないため)
    #[serde(default)]
    pub discord_scheduled_event: Option<bool>,
}

impl EventInput {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.name.trim().is_empty() {
            return Err(ApiError::BadRequest("name is required".into()));
        }
        if self.name.chars().count() > NAME_MAX_CHARS {
            return Err(ApiError::BadRequest(format!(
                "name must be at most {NAME_MAX_CHARS} characters"
            )));
        }
        if let Some(description) = &self.description
            && description.chars().count() > DESCRIPTION_MAX_CHARS
        {
            return Err(ApiError::BadRequest(format!(
                "description must be at most {DESCRIPTION_MAX_CHARS} characters"
            )));
        }
        if !is_hex_color(&self.color) {
            return Err(ApiError::BadRequest(
                "color must be in #RRGGBB format".into(),
            ));
        }
        if self.end_at < self.start_at {
            return Err(ApiError::BadRequest(
                "end_at must not be before start_at".into(),
            ));
        }
        if self.notifications.len() > NOTIFICATIONS_MAX {
            return Err(ApiError::BadRequest(format!(
                "at most {NOTIFICATIONS_MAX} notifications are allowed"
            )));
        }
        Ok(())
    }
}

fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// 通常 API 用: 「Discord のイベントとしても作成する」を有効にできるかの検証 (#94)。
/// Discord は開始時刻が未来のイベントしか作れないため、過去 (現在を含む) 開始でフラグ有効は拒否する。
/// また Discord の外部イベントは終了が開始より後である必要があるため、時刻指定の予定では
/// 通常の検証が許している「同時刻」も拒否する (終日予定は終了に +1 日するので同時刻でよい)。
/// web 側も同じ条件でチェックボックスを制御する。管理コンソールはフラグを無視するのでこの検証を通さない。
/// `discord_scheduled_event` は省略時の解決を済ませた実効値 (呼び出し側が更新では現在の連携状態で補う)
pub fn validate_discord_flag(
    input: &EventInput,
    discord_scheduled_event: bool,
    now: NaiveDateTime,
) -> Result<(), ApiError> {
    if !discord_scheduled_event {
        return Ok(());
    }
    // Discord へ実際に送る開始時刻で判定する。終日予定は時刻を切り捨てた開始日の 0:00 を
    // 送るので (`ScheduledEventPayload::new`)、未正規化の `start_at` で見ると、
    // 時刻付きの終日予定 (API を直接叩いた場合) がここを通ってから Discord に弾かれてしまう
    let start_at = if input.is_all_day {
        input
            .start_at
            .date()
            .and_hms_opt(0, 0, 0)
            .expect("midnight is always valid")
    } else {
        input.start_at
    };
    if start_at <= now {
        return Err(ApiError::BadRequest(
            "start_at must be in the future to create a Discord scheduled event".into(),
        ));
    }
    if !input.is_all_day && input.end_at <= input.start_at {
        return Err(ApiError::BadRequest(
            "end_at must be after start_at to create a Discord scheduled event".into(),
        ));
    }
    // 終日予定は Discord の終了時刻に「終了日の翌日 0:00」を渡すので、翌日が表現できない
    // 終了日 (chrono の上限ぎりぎり) は弾く (通しても Discord が受け付けないうえ、
    // 変換 (`ScheduledEventPayload::new`) が panic してしまう)
    if input.is_all_day && input.end_at.date().succ_opt().is_none() {
        return Err(ApiError::BadRequest(
            "end_at is too far in the future for a Discord scheduled event".into(),
        ));
    }
    Ok(())
}

/// 期間 `[start, end)` に重なる予定 (途中から始まっている複数日の予定も含む)
pub async fn list_between(
    pool: &PgPool,
    guild_id: &str,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> sqlx::Result<Vec<EventRow>> {
    sqlx::query_as!(
        EventRow,
        r#"
        SELECT e.id, e.guild_id, e.name, e.description, e.notifications, e.color, e.is_all_day,
               e.start_at, e.end_at, e.created_at,
               l.scheduled_event_id AS "discord_scheduled_event_id?"
        FROM events e
        LEFT JOIN event_discord_links l ON l.event_id = e.id
        WHERE e.guild_id = $1 AND e.start_at < $3 AND e.end_at >= $2
        ORDER BY e.start_at, e.id
        "#,
        guild_id,
        start,
        end
    )
    .fetch_all(pool)
    .await
}

/// 複数ギルドの、期間 `[start, end)` に重なる予定 (横断カレンダー #98)。
/// `guild_ids` の認可 (Bot 参加済み かつ 呼び出したユーザーがメンバー) は呼び出し側が済ませていること。
/// 並びは [`list_between`] と同じ (開始日時 → id) で、ギルドではまとめない
pub async fn list_between_guilds(
    pool: &PgPool,
    guild_ids: &[String],
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> sqlx::Result<Vec<EventRow>> {
    sqlx::query_as!(
        EventRow,
        r#"
        SELECT e.id, e.guild_id, e.name, e.description, e.notifications, e.color, e.is_all_day,
               e.start_at, e.end_at, e.created_at,
               l.scheduled_event_id AS "discord_scheduled_event_id?"
        FROM events e
        LEFT JOIN event_discord_links l ON l.event_id = e.id
        WHERE e.guild_id = ANY($1) AND e.start_at < $3 AND e.end_at >= $2
        ORDER BY e.start_at, e.id
        "#,
        guild_ids,
        start,
        end
    )
    .fetch_all(pool)
    .await
}

/// iCal フィード (#95) 用: `since` 以降に終わる (または続いている) 予定をすべて返す。
/// 上限は設けない。呼び出し側が「今から 1 年前」を渡すので、行数はギルドの 1 年分 + 未来の予定に収まる。
/// 終日予定の `end_at` は「終了日の 0:00」で、実際にはその翌日 0:00 まで続くので、下限の判定も 1 日ずらす
/// (時刻指定の予定と同じ「実際の終了時刻 >= since」になる)。
/// 認可 (トークンの照合) は呼び出し側が済ませていること
pub async fn list_for_feed(
    pool: &PgPool,
    guild_id: &str,
    since: NaiveDateTime,
) -> sqlx::Result<Vec<EventRow>> {
    sqlx::query_as!(
        EventRow,
        r#"
        SELECT e.id, e.guild_id, e.name, e.description, e.notifications, e.color, e.is_all_day,
               e.start_at, e.end_at, e.created_at,
               l.scheduled_event_id AS "discord_scheduled_event_id?"
        FROM events e
        LEFT JOIN event_discord_links l ON l.event_id = e.id
        WHERE e.guild_id = $1
          AND e.end_at >= CASE WHEN e.is_all_day THEN $2::timestamp - INTERVAL '1 day' ELSE $2::timestamp END
        ORDER BY e.start_at, e.id
        "#,
        guild_id,
        since
    )
    .fetch_all(pool)
    .await
}

/// 返る行の `discord_scheduled_event_id` は常に `None` (対応付けは行を作った後にルート層が
/// [`super::event_links`] へ書き、レスポンスへは呼び出し元が詰め直す)
pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    input: &EventInput,
    created_at: NaiveDateTime,
) -> sqlx::Result<EventRow> {
    let notifications = Notification::encode_all(&input.notifications);
    sqlx::query_as!(
        EventRow,
        r#"
        INSERT INTO events (guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at,
                  NULL::text AS "discord_scheduled_event_id?"
        "#,
        guild_id,
        input.name,
        input.description,
        notifications,
        input.color,
        input.is_all_day,
        input.start_at,
        input.end_at,
        created_at
    )
    .fetch_one(executor)
    .await
}

/// ギルドに属する予定を 1 件、ロックせずに読む (対応付け込み)。該当なしなら `None`。
/// Discord 連携 (#94) の更新で、外部呼び出しの間 DB 接続を占有しないための分岐の起点に使う
/// (実際の書き込みは [`find_by_id_for_update`] でロックを取り直し、ここで読んだ状態と突き合わせる)
pub async fn find_by_id(pool: &PgPool, guild_id: &str, id: i32) -> sqlx::Result<Option<EventRow>> {
    sqlx::query_as!(
        EventRow,
        r#"
        SELECT e.id, e.guild_id, e.name, e.description, e.notifications, e.color, e.is_all_day,
               e.start_at, e.end_at, e.created_at,
               l.scheduled_event_id AS "discord_scheduled_event_id?"
        FROM events e
        LEFT JOIN event_discord_links l ON l.event_id = e.id
        WHERE e.id = $1 AND e.guild_id = $2
        "#,
        id,
        guild_id
    )
    .fetch_optional(pool)
    .await
}

/// ギルドに属する予定を 1 件取得し、トランザクションの終わりまで行をロックする (`FOR UPDATE`)。
/// 管理コンソールが監査ログの「変更前」として読むのと、Discord 連携 (#94) の分岐の起点に使う。
/// 更新・削除までの間に別トランザクションが同じ行を書き換えて、before と実際の直前の値が
/// ずれるのを防ぐ。他ギルドの ID を指定しても返さない。該当なしなら `None`。
///
/// 対応付け (`event_discord_links`) は**ロックを取った後に別の文で**読む。
/// ロックと同じ文で JOIN すると、ロック待ちの間に別トランザクションが足した対応付けが
/// 文の開始時点のスナップショットに含まれず、古い (無い) ように見えてしまうため
pub async fn find_by_id_for_update(
    conn: &mut sqlx::PgConnection,
    guild_id: &str,
    id: i32,
) -> sqlx::Result<Option<EventRow>> {
    let row = sqlx::query_as!(
        EventRow,
        r#"
        SELECT id, guild_id, name, description, notifications, color, is_all_day,
               start_at, end_at, created_at,
               NULL::text AS "discord_scheduled_event_id?"
        FROM events
        WHERE id = $1 AND guild_id = $2
        FOR UPDATE
        "#,
        id,
        guild_id
    )
    .fetch_optional(&mut *conn)
    .await?;
    let Some(mut row) = row else {
        return Ok(None);
    };
    row.discord_scheduled_event_id = super::event_links::get(&mut *conn, guild_id, id).await?;
    Ok(Some(row))
}

/// 対応付け (`event_discord_links`) が無いときだけ更新する (#94)。
/// 連携も解除も伴わない通常の更新 (ドラッグ移動など) の速い経路で使い、
/// 直前に別リクエストが連携を足していた場合は更新せずに `None` を返す
/// (呼び出し側は連携ありの経路で処理をやり直す)。
/// 予定が存在しないときも `None` なので、区別が要る呼び出し側は別途確認する
pub async fn update_if_unlinked<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    id: i32,
    input: &EventInput,
) -> sqlx::Result<Option<EventRow>> {
    let notifications = Notification::encode_all(&input.notifications);
    sqlx::query_as!(
        EventRow,
        r#"
        UPDATE events
        SET name = $3, description = $4, notifications = $5, color = $6, is_all_day = $7, start_at = $8, end_at = $9
        WHERE id = $1 AND guild_id = $2
          AND NOT EXISTS (SELECT 1 FROM event_discord_links l WHERE l.event_id = events.id)
        RETURNING id, guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at,
                  NULL::text AS "discord_scheduled_event_id?"
        "#,
        id,
        guild_id,
        input.name,
        input.description,
        notifications,
        input.color,
        input.is_all_day,
        input.start_at,
        input.end_at
    )
    .fetch_optional(executor)
    .await
}

/// ギルドに属する予定だけを更新する (他ギルドの ID を指定しても更新されない)。該当なしなら `None`。
/// 書き込み関数は executor を受け取るので、管理コンソールからは監査ログと同じトランザクションで呼べる。
/// 返る行の `discord_scheduled_event_id` は常に `None` ([`create`] と同じ理由)
pub async fn update<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    id: i32,
    input: &EventInput,
) -> sqlx::Result<Option<EventRow>> {
    let notifications = Notification::encode_all(&input.notifications);
    sqlx::query_as!(
        EventRow,
        r#"
        UPDATE events
        SET name = $3, description = $4, notifications = $5, color = $6, is_all_day = $7, start_at = $8, end_at = $9
        WHERE id = $1 AND guild_id = $2
        RETURNING id, guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at,
                  NULL::text AS "discord_scheduled_event_id?"
        "#,
        id,
        guild_id,
        input.name,
        input.description,
        notifications,
        input.color,
        input.is_all_day,
        input.start_at,
        input.end_at
    )
    .fetch_optional(executor)
    .await
}

/// 削除できたら `true`
pub async fn delete<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    id: i32,
) -> sqlx::Result<bool> {
    let result = sqlx::query!(
        "DELETE FROM events WHERE id = $1 AND guild_id = $2",
        id,
        guild_id
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::notifications::NotificationUnit;

    fn input() -> EventInput {
        EventInput {
            name: "test".into(),
            description: None,
            notifications: vec![],
            color: "#2196F3".into(),
            is_all_day: false,
            start_at: "2026-08-22T10:00:00".parse().unwrap(),
            end_at: "2026-08-22T11:00:00".parse().unwrap(),
            discord_scheduled_event: None,
        }
    }

    #[test]
    fn valid_input_passes() {
        assert!(input().validate().is_ok());
    }

    #[test]
    fn rejects_blank_name() {
        let mut i = input();
        i.name = "   ".into();
        assert!(matches!(i.validate(), Err(ApiError::BadRequest(_))));
    }

    #[test]
    fn counts_name_length_in_chars() {
        let mut i = input();
        i.name = "あ".repeat(NAME_MAX_CHARS);
        assert!(i.validate().is_ok());
        i.name = "あ".repeat(NAME_MAX_CHARS + 1);
        assert!(i.validate().is_err());
    }

    #[test]
    fn rejects_bad_color() {
        for color in ["2196F3", "#2196F", "#GGGGGG", "#2196F3FF"] {
            let mut i = input();
            i.color = color.into();
            assert!(i.validate().is_err(), "{color}");
        }
    }

    #[test]
    fn rejects_end_before_start() {
        let mut i = input();
        i.end_at = "2026-08-22T09:59:59".parse().unwrap();
        assert!(i.validate().is_err());
        // 同時刻は許可 (終日予定など)
        i.end_at = i.start_at;
        assert!(i.validate().is_ok());
    }

    #[test]
    fn discord_flag_requires_future_start() {
        let now = "2026-08-22T10:00:00".parse().unwrap();
        let i = input();
        // フラグが無効なら過去開始でも通る
        assert!(validate_discord_flag(&i, false, now).is_ok());
        // 開始が現在以前なら拒否 (Discord は未来の開始時刻を必須とする)
        assert!(validate_discord_flag(&i, true, now).is_err());
        assert!(validate_discord_flag(&i, true, "2026-08-22T09:59:59".parse().unwrap()).is_ok());
    }

    #[test]
    fn discord_flag_checks_the_normalized_start_for_all_day_events() {
        // 終日予定は開始日の 0:00 を Discord へ送るので、時刻部分が未来でも
        // その日の 0:00 が過ぎていれば連携できない (API を直接叩いた場合の入力)
        let mut i = input();
        i.is_all_day = true;
        i.start_at = "2026-08-22T23:00:00".parse().unwrap();
        i.end_at = "2026-08-22T23:00:00".parse().unwrap();
        let noon = "2026-08-22T12:00:00".parse().unwrap();
        assert!(validate_discord_flag(&i, true, noon).is_err());
        // 前日のうちなら通る (0:00 がまだ未来)
        let day_before = "2026-08-21T23:59:59".parse().unwrap();
        assert!(validate_discord_flag(&i, true, day_before).is_ok());
    }

    #[test]
    fn discord_flag_requires_end_after_start_for_timed_events() {
        let now = "2026-08-01T00:00:00".parse().unwrap();
        let mut i = input();
        i.end_at = i.start_at;
        // 時刻指定の同時刻は通常の検証は許すが、Discord の外部イベントは作れないので拒否
        assert!(i.validate().is_ok());
        assert!(validate_discord_flag(&i, true, now).is_err());
        // 終日予定は終了に +1 日するので同時刻でよい
        i.is_all_day = true;
        assert!(validate_discord_flag(&i, true, now).is_ok());
        // フラグが無効なら関知しない
        i.is_all_day = false;
        assert!(validate_discord_flag(&i, false, now).is_ok());
    }

    #[test]
    fn discord_flag_rejects_all_day_end_at_date_max() {
        // chrono の上限日は「翌日 0:00」が作れず変換が panic するので、検証で弾く
        let now = "2026-08-01T00:00:00".parse().unwrap();
        let mut i = input();
        i.is_all_day = true;
        i.end_at = chrono::NaiveDate::MAX.and_hms_opt(0, 0, 0).unwrap();
        assert!(i.validate().is_ok());
        assert!(validate_discord_flag(&i, true, now).is_err());
    }

    #[test]
    fn rejects_too_many_notifications() {
        let mut i = input();
        i.notifications = vec![
            Notification {
                num: 1,
                unit: NotificationUnit::Minutes
            };
            NOTIFICATIONS_MAX + 1
        ];
        assert!(i.validate().is_err());
    }
}
