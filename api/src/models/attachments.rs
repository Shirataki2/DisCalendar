//! 添付の予約・容量制限。予約から確定まで同じ行を使い、未確定分も上限に数える。
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool};
use utoipa::ToSchema;

use crate::error::ApiError;

pub const FILE_MAX_BYTES: i64 = 10 * 1024 * 1024;
pub const EVENT_MAX_FILES: i64 = 10;
pub const GUILD_MAX_BYTES: i64 = 1024 * 1024 * 1024;

#[derive(Debug, Serialize, sqlx::FromRow, ToSchema)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub size: i64,
    pub content_type: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip)]
    #[schema(ignore)]
    pub temporary_key: String,
    #[serde(skip)]
    #[schema(ignore)]
    pub object_key: String,
    #[serde(skip)]
    #[schema(ignore)]
    pub ready: bool,
    #[serde(skip)]
    #[schema(ignore)]
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UploadInput {
    pub filename: String,
    pub size: i64,
    pub content_type: String,
}
impl UploadInput {
    pub fn validate(&self) -> Result<(), ApiError> {
        let extension = self
            .filename
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let valid_type = match self.content_type.as_str() {
            "image/jpeg" => matches!(extension.as_str(), "jpg" | "jpeg"),
            "image/png" => extension == "png",
            "image/webp" => extension == "webp",
            "application/pdf" => extension == "pdf",
            _ => false,
        };
        if !valid_type
            || self.filename.trim().is_empty()
            || self.filename.len() > 255
            || self
                .filename
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\'))
        {
            return Err(ApiError::BadRequest(
                "JPEG・PNG・WebP・PDFを有効なファイル名で指定してください".into(),
            ));
        }
        if !(1..=FILE_MAX_BYTES).contains(&self.size) {
            return Err(ApiError::BadRequest(
                "ファイルは1バイト以上10MiB以下にしてください".into(),
            ));
        }
        Ok(())
    }
}

pub async fn used_bytes(pool: &PgPool, guild_id: &str) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "SELECT COALESCE(sum(size),0)::bigint FROM event_attachments WHERE guild_id=$1",
    )
    .bind(guild_id)
    .fetch_one(pool)
    .await
}

pub async fn list(pool: &PgPool, guild_id: &str, event_id: i32) -> sqlx::Result<Vec<Attachment>> {
    sqlx::query_as("SELECT * FROM event_attachments WHERE guild_id=$1 AND event_id=$2 AND ready ORDER BY created_at,id")
        .bind(guild_id).bind(event_id).fetch_all(pool).await
}

/// ロック順は予定 → 添付。予定削除の CASCADE と確定がデッドロックしないよう統一する。
pub async fn lock_event(
    conn: &mut PgConnection,
    guild_id: &str,
    event_id: i32,
) -> Result<(), ApiError> {
    let id: Option<i32> =
        sqlx::query_scalar("SELECT id FROM events WHERE guild_id=$1 AND id=$2 FOR UPDATE")
            .bind(guild_id)
            .bind(event_id)
            .fetch_optional(conn)
            .await?;
    id.ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    Ok(())
}

pub async fn reserve(
    pool: &PgPool,
    guild_id: &str,
    event_id: i32,
    input: &UploadInput,
) -> Result<Attachment, ApiError> {
    input.validate()?;
    let mut tx = pool.begin().await?;
    // 別予定への予約もサーバー単位で直列化する。退出後も残る予定を扱うので guilds 行には依存しない。
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 245))")
        .bind(guild_id)
        .execute(&mut *tx)
        .await?;
    lock_event(&mut tx, guild_id, event_id).await?;
    let (count, bytes): (i64, i64) = sqlx::query_as("SELECT count(*) FILTER (WHERE event_id=$2), COALESCE(sum(size),0)::bigint FROM event_attachments WHERE guild_id=$1")
        .bind(guild_id).bind(event_id).fetch_one(&mut *tx).await?;
    if count >= EVENT_MAX_FILES || bytes + input.size > GUILD_MAX_BYTES {
        return Err(ApiError::BadRequest(
            "添付は予定ごとに10件、サーバー全体で1GiBまでです（送信待ちを含む）".into(),
        ));
    }
    let id = format!("{:032x}", rand::random::<u128>());
    let row = sqlx::query_as("INSERT INTO event_attachments (id,event_id,guild_id,filename,size,content_type,temporary_key,object_key) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING *")
        .bind(&id).bind(event_id).bind(guild_id).bind(&input.filename).bind(input.size).bind(&input.content_type)
        .bind(format!("temporary/{id}")).bind(format!("attachments/{id}")).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn find(
    conn: &mut PgConnection,
    guild_id: &str,
    event_id: i32,
    id: &str,
) -> Result<Attachment, ApiError> {
    sqlx::query_as(
        "SELECT * FROM event_attachments WHERE guild_id=$1 AND event_id=$2 AND id=$3 FOR UPDATE",
    )
    .bind(guild_id)
    .bind(event_id)
    .bind(id)
    .fetch_optional(conn)
    .await?
    .ok_or_else(|| ApiError::NotFound("attachment not found".into()))
}

/// マジックバイトを調べる。PDFはダウンロード専用で、スクリプトを含み得る文書を埋め込まない。
pub fn detected_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else if bytes.starts_with(b"%PDF-") {
        Some("application/pdf")
    } else {
        None
    }
}
