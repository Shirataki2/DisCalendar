use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::error::ApiError;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
pub struct DigestSettings {
    pub daily_enabled: bool,
    /// JST の HH:mm (00:00〜23:59)
    pub daily_time: String,
    pub weekly_enabled: bool,
    /// 月曜 = 0、日曜 = 6
    pub weekly_day: i16,
    /// JST の HH:mm (00:00〜23:59)
    pub weekly_time: String,
    pub skip_empty: bool,
}

impl Default for DigestSettings {
    fn default() -> Self {
        Self {
            daily_enabled: false,
            daily_time: "08:00".into(),
            weekly_enabled: false,
            weekly_day: 0,
            weekly_time: "08:00".into(),
            skip_empty: true,
        }
    }
}

impl DigestSettings {
    pub fn validate(&self) -> Result<(NaiveTime, NaiveTime), ApiError> {
        let parse = |value: &str| {
            NaiveTime::parse_from_str(value, "%H:%M")
                .ok()
                .filter(|time| time.format("%H:%M").to_string() == value)
                .ok_or_else(|| ApiError::BadRequest("time must be HH:mm (00:00–23:59 JST)".into()))
        };
        if !(0..=6).contains(&self.weekly_day) {
            return Err(ApiError::BadRequest(
                "weekly_day must be 0–6 (Monday–Sunday)".into(),
            ));
        }
        Ok((parse(&self.daily_time)?, parse(&self.weekly_time)?))
    }
}

pub async fn get(pool: &PgPool, guild_id: &str) -> sqlx::Result<DigestSettings> {
    Ok(sqlx::query_as!(DigestSettings, r#"
        SELECT daily_enabled, to_char(daily_time, 'HH24:MI') AS "daily_time!",
               weekly_enabled, weekly_day, to_char(weekly_time, 'HH24:MI') AS "weekly_time!", skip_empty
        FROM guild_digest_settings WHERE guild_id = $1
    "#, guild_id).fetch_optional(pool).await?.unwrap_or_default())
}

pub async fn put(pool: &PgPool, guild_id: &str, input: &DigestSettings) -> Result<(), ApiError> {
    let (daily_time, weekly_time) = input.validate()?;
    sqlx::query!(r#"
        INSERT INTO guild_digest_settings
            (guild_id, daily_enabled, daily_time, weekly_enabled, weekly_day, weekly_time, skip_empty)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (guild_id) DO UPDATE SET
            daily_enabled = EXCLUDED.daily_enabled, daily_time = EXCLUDED.daily_time,
            weekly_enabled = EXCLUDED.weekly_enabled, weekly_day = EXCLUDED.weekly_day,
            weekly_time = EXCLUDED.weekly_time, skip_empty = EXCLUDED.skip_empty
    "#, guild_id, input.daily_enabled, daily_time, input.weekly_enabled, input.weekly_day,
        weekly_time, input.skip_empty).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_time_and_weekday_even_when_disabled() {
        let mut settings = DigestSettings::default();
        assert!(settings.validate().is_ok());
        for time in ["00:00", "23:59"] {
            settings.daily_time = time.into();
            settings.weekly_time = time.into();
            assert!(settings.validate().is_ok());
        }
        for time in ["24:00", "08:60", "8:00", "08:00:00", "", " 08:00"] {
            settings.daily_time = time.into();
            assert!(settings.validate().is_err());
            settings.daily_time = "08:00".into();
            settings.weekly_time = time.into();
            assert!(settings.validate().is_err());
        }
        settings.weekly_time = "08:00".into();
        for day in [-1, 7] {
            settings.weekly_day = day;
            assert!(settings.validate().is_err());
        }
        for day in [0, 6] {
            settings.weekly_day = day;
            assert!(settings.validate().is_ok());
        }
    }
}
