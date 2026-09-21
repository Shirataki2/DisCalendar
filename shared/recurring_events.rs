//! 各開催回の補充と例外記録。API・Botは同じ行ロックと一意制約を使う。
use crate::recurrence::{self, Rule};
use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgConnection, PgPool};

#[derive(Debug, Clone, Deserialize, sqlx::FromRow)]
pub struct Series {
    pub id: i32,
    pub guild_id: String,
    pub template: Value,
    pub recurrence: Value,
    pub rrule: String,
    pub start_at: NaiveDateTime,
    pub end_at: NaiveDateTime,
    pub end_before: Option<NaiveDateTime>,
    pub version: i32,
    pub created_at: NaiveDateTime,
    pub created_by: String,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
    pub generated_until: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Info {
    pub series_id: i32,
    pub original_start_at: NaiveDateTime,
    pub is_exception: bool,
    pub rule: Value,
    pub version: i32,
}

pub async fn info(conn: &mut PgConnection, guild: &str, event: i32) -> sqlx::Result<Option<Info>> {
    sqlx::query_as("SELECT s.id AS series_id,e.original_start_at,EXISTS(SELECT 1 FROM event_series_exceptions x WHERE x.event_id=e.id) AS is_exception,s.recurrence AS rule,s.version FROM events e JOIN event_series s ON s.id=e.series_id AND s.guild_id=e.guild_id WHERE e.guild_id=$1 AND e.id=$2")
        .bind(guild).bind(event).fetch_optional(conn).await
}

pub async fn lock_for_event(
    conn: &mut PgConnection,
    guild: &str,
    event: i32,
) -> sqlx::Result<Option<Series>> {
    // 分割を待っている間に所属シリーズが変わった場合、新しいシリーズのロックを取り直す。
    loop {
        let id: Option<i32> =
            sqlx::query_scalar("SELECT series_id FROM events WHERE guild_id=$1 AND id=$2")
                .bind(guild)
                .bind(event)
                .fetch_optional(&mut *conn)
                .await?
                .flatten();
        let Some(id) = id else { return Ok(None) };
        let series: Option<Series> =
            sqlx::query_as("SELECT * FROM event_series WHERE guild_id=$1 AND id=$2 FOR UPDATE")
                .bind(guild)
                .bind(id)
                .fetch_optional(&mut *conn)
                .await?;
        let current: Option<i32> =
            sqlx::query_scalar("SELECT series_id FROM events WHERE guild_id=$1 AND id=$2")
                .bind(guild)
                .bind(event)
                .fetch_optional(&mut *conn)
                .await?
                .flatten();
        if current == Some(id) {
            return Ok(series);
        }
    }
}

pub async fn record_exception(
    conn: &mut PgConnection,
    guild: &str,
    event: i32,
    cancelled: bool,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO event_series_exceptions (series_id,original_start_at,event_id) SELECT series_id,original_start_at,CASE WHEN $3 THEN NULL ELSE id END FROM events WHERE guild_id=$1 AND id=$2 AND series_id IS NOT NULL ON CONFLICT (series_id,original_start_at) DO UPDATE SET event_id=EXCLUDED.event_id")
        .bind(guild).bind(event).bind(cancelled).execute(&mut *conn).await?;
    sqlx::query("UPDATE event_series SET version=version+1 WHERE id=(SELECT series_id FROM events WHERE guild_id=$1 AND id=$2)")
        .bind(guild).bind(event).execute(conn).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn create_series(
    conn: &mut PgConnection,
    guild: &str,
    template: Value,
    rule: &Rule,
    start: NaiveDateTime,
    end: NaiveDateTime,
    actor: &str,
    now: NaiveDateTime,
) -> Result<Series, anyhow::Error> {
    let mut rule = rule.clone();
    if let Rule::Weekly { weekdays, .. } | Rule::Biweekly { weekdays, .. } = &mut rule {
        weekdays.sort_unstable();
        weekdays.dedup();
    }
    let rrule = rule.rrule(start).map_err(anyhow::Error::msg)?;
    Ok(sqlx::query_as("INSERT INTO event_series (guild_id,template,recurrence,rrule,start_at,end_at,created_by,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING *")
        .bind(guild).bind(template).bind(serde_json::to_value(&rule)?).bind(rrule).bind(start).bind(end).bind(actor).bind(now).fetch_one(conn).await?)
}

pub async fn insert_occurrence(
    conn: &mut PgConnection,
    series: &Series,
    start: NaiveDateTime,
) -> Result<(), anyhow::Error> {
    let end = start
        .checked_add_signed(series.end_at - series.start_at)
        .ok_or_else(|| anyhow::anyhow!("開催終了日時が範囲外です"))?;
    sqlx::query("INSERT INTO events (guild_id,name,description,notifications,notification_mentions,color,is_all_day,start_at,end_at,created_at,created_by,updated_at,updated_by,series_id,original_start_at) SELECT $1,$2::jsonb->>'name',$2::jsonb->>'description',COALESCE($2::jsonb->'notifications','[]'::jsonb),COALESCE(NULLIF($2::jsonb->'notification_mentions','null'::jsonb),'[]'::jsonb),$2::jsonb->>'color',($2::jsonb->>'is_all_day')::boolean,$3,$4,$5,$6,$7,$8,$9,$3 WHERE NOT EXISTS (SELECT 1 FROM event_series_exceptions WHERE series_id=$9 AND original_start_at=$3) ON CONFLICT (series_id,original_start_at) DO NOTHING")
        .bind(&series.guild_id).bind(&series.template).bind(start).bind(end).bind(series.created_at).bind(&series.created_by)
        .bind(series.updated_at).bind(&series.updated_by).bind(series.id).execute(conn).await?;
    Ok(())
}

pub async fn fill(
    conn: &mut PgConnection,
    series: &Series,
    from: NaiveDateTime,
    to: NaiveDateTime,
) -> Result<(), anyhow::Error> {
    let duration = series.end_at - series.start_at;
    // 終日の包含終了日と、表示範囲より前から続いている予定も拾う。
    let overlap = if series.template["is_all_day"] == true {
        duration + Duration::days(1)
    } else {
        duration
    };
    let from = from
        .checked_sub_signed(overlap)
        .ok_or_else(|| anyhow::anyhow!("取得範囲が広すぎます"))?
        .max(series.start_at);
    let to = series.end_before.map_or(to, |end| to.min(end));
    for start in
        recurrence::between(&series.rrule, series.start_at, from, to).map_err(anyhow::Error::msg)?
    {
        insert_occurrence(conn, series, start).await?;
    }
    Ok(())
}

pub async fn ensure_range(
    pool: &PgPool,
    guild: &str,
    from: NaiveDateTime,
    to: NaiveDateTime,
) -> Result<(), anyhow::Error> {
    let mut tx = pool.begin().await?;
    let series: Vec<Series> = sqlx::query_as(
        "SELECT * FROM event_series WHERE guild_id=$1 AND start_at < $2 ORDER BY id FOR UPDATE",
    )
    .bind(guild)
    .bind(to)
    .fetch_all(&mut *tx)
    .await?;
    for s in series {
        fill(&mut tx, &s, from, to).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// 日ごとに先行期間を延長する。全期間を毎分再生成しない。
pub async fn replenish(pool: &PgPool, now: NaiveDateTime) -> Result<(), anyhow::Error> {
    let from = now.date().and_hms_opt(0, 0, 0).expect("midnight");
    let to = from + Duration::days(recurrence::LOOKAHEAD_DAYS + 1);
    let ids: Vec<i32> = sqlx::query_scalar("SELECT id FROM event_series WHERE start_at < $1 AND (end_before IS NULL OR end_before > $2) AND (generated_until IS NULL OR generated_until < $1) ORDER BY id")
        .bind(to).bind(from).fetch_all(pool).await?;
    for id in ids {
        let mut tx = pool.begin().await?;
        let s: Option<Series> = sqlx::query_as("SELECT * FROM event_series WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(s) = s {
            fill(&mut tx, &s, s.generated_until.unwrap_or(from).max(from), to).await?;
            sqlx::query("UPDATE event_series SET generated_until=$2 WHERE id=$1")
                .bind(id)
                .bind(to)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
    }
    Ok(())
}
