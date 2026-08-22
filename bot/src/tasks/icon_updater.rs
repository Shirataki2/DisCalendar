//! 毎日 JST 0:00 に Bot 本体とサポートサーバーのアイコンを日付入りのものへ差し替える
//! (旧 `tasks/icon_updater.rs` 相当)。
//!
//! 画像は `bot/assets/{DD}.png` (平日) / `{DD}_b.png` (土曜) / `{DD}_r.png` (日曜・祝日)
//! (`tmp/DisCalendarV2/bot/assets/` からコピーしたもの)。

use std::{path::PathBuf, time::Duration as StdDuration};

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike, Weekday};
use jpholiday::Date as JpDate;
use poise::serenity_prelude::{self as serenity, CreateAttachment, EditGuild, EditProfile};

use crate::{data::Data, models::now_jst};

const INTERVAL: StdDuration = StdDuration::from_secs(60);
const ASSETS_DIR: &str = "assets";

pub async fn run_loop(ctx: serenity::Context, data: Data) {
    loop {
        let now = now_jst();
        if now.hour() == 0 && now.minute() == 0 {
            run_once(&ctx, &data, now).await;
        }
        tokio::time::sleep(INTERVAL).await;
    }
}

async fn run_once(ctx: &serenity::Context, data: &Data, now: NaiveDateTime) {
    let path = icon_path(now.date());
    let attachment = match CreateAttachment::path(&path).await {
        Ok(attachment) => attachment,
        Err(e) => {
            tracing::error!(error = %e, path = %path.display(), "failed to read date icon");
            return;
        }
    };

    let mut me = ctx.cache.current_user().clone();
    match me.edit(ctx, EditProfile::new().avatar(&attachment)).await {
        Ok(()) => tracing::info!("updated bot avatar"),
        Err(e) => tracing::warn!(error = %e, "failed to update bot avatar"),
    }

    let Some(support_guild_id) = data.support_guild_id else {
        return;
    };
    match support_guild_id
        .edit(&ctx.http, EditGuild::new().icon(Some(&attachment)))
        .await
    {
        Ok(_) => tracing::info!("updated support guild icon"),
        Err(e) => tracing::warn!(error = %e, "failed to update support guild icon"),
    }
}

/// 日付・曜日・祝日から使う画像ファイルを選ぶ (祝日または日曜は `_r`、土曜は `_b`、それ以外は無印)
fn icon_path(date: NaiveDate) -> PathBuf {
    let day = date.format("%d");
    let suffix = if is_holiday(date) || date.weekday() == Weekday::Sun {
        "_r"
    } else if date.weekday() == Weekday::Sat {
        "_b"
    } else {
        ""
    };
    PathBuf::from(ASSETS_DIR).join(format!("{day}{suffix}.png"))
}

fn is_holiday(date: NaiveDate) -> bool {
    match JpDate::new(date.year(), date.month(), date.day()) {
        Ok(jp_date) => jpholiday::is_holiday(jp_date),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn picks_plain_icon_on_weekdays() {
        // 2026-08-24 は月曜、祝日でもない
        assert_eq!(
            icon_path(date("2026-08-24")),
            PathBuf::from("assets/24.png")
        );
    }

    #[test]
    fn picks_saturday_icon() {
        // 2026-08-22 は土曜
        assert_eq!(
            icon_path(date("2026-08-22")),
            PathBuf::from("assets/22_b.png")
        );
    }

    #[test]
    fn picks_sunday_icon() {
        // 2026-08-23 は日曜
        assert_eq!(
            icon_path(date("2026-08-23")),
            PathBuf::from("assets/23_r.png")
        );
    }

    #[test]
    fn picks_holiday_icon_even_on_a_weekday() {
        // 2026-01-01 は元日 (木曜)
        assert_eq!(
            icon_path(date("2026-01-01")),
            PathBuf::from("assets/01_r.png")
        );
    }
}
