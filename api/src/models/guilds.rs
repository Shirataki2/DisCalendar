use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgExecutor, PgPool};
use utoipa::ToSchema;

use super::notifications::{Notification, NotificationUnit};

/// Bot が参加しているギルド (`guilds` テーブル。Bot が参加/更新時に書き込む)
#[derive(Debug, Serialize, ToSchema)]
pub struct Guild {
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    #[schema(example = "ja")]
    pub locale: String,
}

pub async fn find_by_guild_id(pool: &PgPool, guild_id: &str) -> sqlx::Result<Option<Guild>> {
    sqlx::query_as!(
        Guild,
        "SELECT guild_id, name, avatar_url, locale FROM guilds WHERE guild_id = $1",
        guild_id
    )
    .fetch_optional(pool)
    .await
}

/// 指定した ID のうち Bot が参加しているギルド
pub async fn find_joined(pool: &PgPool, guild_ids: &[String]) -> sqlx::Result<Vec<Guild>> {
    sqlx::query_as!(
        Guild,
        "SELECT guild_id, name, avatar_url, locale FROM guilds WHERE guild_id = ANY($1) ORDER BY name",
        guild_ids
    )
    .fetch_all(pool)
    .await
}

/// 設定していないサーバーの「新しい予定の既定の事前通知」(#181)。
/// web のフォームがもともと持っていた初期値 (1 日前と 1 時間前) と同じで、
/// マイグレーション (`20260906111934`) の `default_notifications` の DEFAULT とも同じ値
pub const DEFAULT_NOTIFICATIONS: [Notification; 2] = [
    Notification {
        num: 1,
        unit: NotificationUnit::Days,
    },
    Notification {
        num: 1,
        unit: NotificationUnit::Hours,
    },
];

/// ギルドごとの設定。`restricted` / `notify_at_start` / `default_notifications` は `guild_config` テーブル、
/// `notification_channel_id` は `/init` と共有の `event_settings` テーブル (#181)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GuildConfig {
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    /// true の場合、予定の追加・編集・削除を管理権限
    /// (管理者 / サーバー管理 / メッセージの管理 / ロールの管理) を持つユーザーに限定する
    pub restricted: bool,
    /// 予定の開始時刻に通知するか (#181)。false なら Bot は事前通知だけを送る。既定は true
    pub notify_at_start: bool,
    /// web で予定を新規作成するときの事前通知の初期値 (#181)。既定は 1 日前と 1 時間前
    pub default_notifications: Vec<Notification>,
    /// 通知先チャンネルの ID (`/init` またはサーバー設定で設定する)。未設定なら null で、通知は届かない
    #[schema(example = "782502586817314820")]
    pub notification_channel_id: Option<String>,
}

/// 未設定なら既定値 (restricted = false、開始時刻に通知する、既定の事前通知は
/// [`DEFAULT_NOTIFICATIONS`]、通知先なし) を返す。読み取りでは行を作らない。
/// 通知先は旧スキーマに `guild_id` の一意制約が無いので、Bot (`event_settings::get`) と同じく先頭の 1 行を使う
pub async fn get_config<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
) -> sqlx::Result<GuildConfig> {
    let row = sqlx::query!(
        r#"
        SELECT
            COALESCE(gc.restricted, FALSE) AS "restricted!",
            COALESCE(gc.notify_at_start, TRUE) AS "notify_at_start!",
            COALESCE(gc.default_notifications, $2::jsonb) AS "default_notifications!",
            es.channel_id AS "notification_channel_id?"
        FROM (SELECT $1::text AS guild_id) g
        LEFT JOIN guild_config gc ON gc.guild_id = g.guild_id
        LEFT JOIN LATERAL (
            SELECT channel_id FROM event_settings WHERE guild_id = g.guild_id ORDER BY id LIMIT 1
        ) es ON TRUE
        "#,
        guild_id,
        Notification::encode_all(&DEFAULT_NOTIFICATIONS)
    )
    .fetch_one(executor)
    .await?;
    Ok(GuildConfig {
        guild_id: guild_id.to_owned(),
        restricted: row.restricted,
        notify_at_start: row.notify_at_start,
        default_notifications: Notification::decode_all(&row.default_notifications),
        notification_channel_id: row.notification_channel_id,
    })
}

/// 監査ログの「変更前」として設定を読み、同じトランザクションの終わりまで行をロックする。
/// 行がまだ無いギルドでは `FOR UPDATE` だけだと何もロックできず、読み取りから upsert までの間に
/// 通常 API や別の管理リクエストが最初の行を作ると before (既定値) と実際の直前値がずれるので、
/// 先に既定値の行を `INSERT ... ON CONFLICT DO NOTHING` で確保してから `FOR UPDATE` で読む
/// (並行する upsert はこのトランザクションの完了まで行ロックで待つ。トランザクションを
/// ロールバックすれば確保した行も消える)。トランザクション内で呼ぶこと
pub async fn lock_config_for_update(
    conn: &mut PgConnection,
    guild_id: &str,
) -> sqlx::Result<GuildConfig> {
    sqlx::query!(
        "INSERT INTO guild_config (guild_id) VALUES ($1) ON CONFLICT (guild_id) DO NOTHING",
        guild_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "SELECT guild_id FROM guild_config WHERE guild_id = $1 FOR UPDATE",
        guild_id
    )
    .fetch_one(&mut *conn)
    .await?;
    // スナップショットには通知先 (event_settings) も入るので、`/init` や web の通知先変更が
    // 変更前と変更後の読み取りの間に割り込んで「管理者が通知先も変えた」ように記録されないよう、
    // 通知先の書き込みと同じアドバイザリロックも取っておく (トランザクション終了で外れる)
    lock_notification_channel(&mut *conn, guild_id).await?;
    get_config(&mut *conn, guild_id).await
}

/// 通知先 (`event_settings`) の読み書きを直列化するギルド ID ごとのアドバイザリロック。
/// Bot の `/init` (`event_settings::set`) と同じキー。トランザクション内で呼ぶこと
async fn lock_notification_channel(conn: &mut PgConnection, guild_id: &str) -> sqlx::Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(guild_id)
        .execute(conn)
        .await?;
    Ok(())
}

/// [`upsert_config`] で変える項目。`None` の項目は今の値のまま
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildConfigUpdate {
    pub restricted: bool,
    pub notify_at_start: Option<bool>,
    pub default_notifications: Option<Vec<Notification>>,
}

impl GuildConfigUpdate {
    /// restricted だけを変える (管理コンソール用)
    pub fn restricted(restricted: bool) -> Self {
        Self {
            restricted,
            ..Self::default()
        }
    }
}

/// `guild_config` の行を作る・更新する。通知先 (`event_settings`) は含まない ([`set_notification_channel`])。
/// 監査ログと同じトランザクションで呼べるよう接続を受け取る。結果は [`get_config`] で読み直す
pub async fn upsert_config(
    conn: &mut PgConnection,
    guild_id: &str,
    update: &GuildConfigUpdate,
) -> sqlx::Result<()> {
    let default_notifications = update
        .default_notifications
        .as_deref()
        .map(Notification::encode_all);
    sqlx::query!(
        r#"
        INSERT INTO guild_config (guild_id, restricted, notify_at_start, default_notifications)
        VALUES ($1, $2, COALESCE($3::boolean, TRUE), COALESCE($4::jsonb, $5::jsonb))
        ON CONFLICT (guild_id) DO UPDATE SET
            restricted = EXCLUDED.restricted,
            notify_at_start = COALESCE($3::boolean, guild_config.notify_at_start),
            default_notifications = COALESCE($4::jsonb, guild_config.default_notifications)
        "#,
        guild_id,
        update.restricted,
        update.notify_at_start,
        default_notifications,
        Notification::encode_all(&DEFAULT_NOTIFICATIONS)
    )
    .execute(conn)
    .await?;
    Ok(())
}

/// 通知先チャンネルを設定する (`/init` と同じ `event_settings` の行を書く、#181)。
/// 戻り値は変更前のチャンネル ID (初回設定なら `None`)。
///
/// 旧スキーマには `guild_id` の一意制約が無いので `ON CONFLICT` は使えない。web と `/init` が同時に
/// 走ったときに両方が「未設定」と判断して 2 行 INSERT しないよう、Bot (`event_settings::set`) と
/// 同じギルド ID ごとのアドバイザリロックで読み取りから書き込みまでを直列化する
/// (ロックはトランザクション終了時に自動で外れる)。トランザクション内で呼ぶこと
pub async fn set_notification_channel(
    conn: &mut PgConnection,
    guild_id: &str,
    channel_id: &str,
) -> sqlx::Result<Option<String>> {
    lock_notification_channel(&mut *conn, guild_id).await?;
    let previous = sqlx::query_scalar!(
        "SELECT channel_id FROM event_settings WHERE guild_id = $1 ORDER BY id LIMIT 1",
        guild_id
    )
    .fetch_optional(&mut *conn)
    .await?;
    if previous.is_some() {
        // 旧 Bot の時代に同じギルドの行が複数できていれば、まとめて更新する
        sqlx::query!(
            "UPDATE event_settings SET channel_id = $2 WHERE guild_id = $1",
            guild_id,
            channel_id
        )
        .execute(&mut *conn)
        .await?;
    } else {
        sqlx::query!(
            "INSERT INTO event_settings (guild_id, channel_id) VALUES ($1, $2)",
            guild_id,
            channel_id
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(previous)
}
