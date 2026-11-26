//! スタッフ用のスラッシュコマンド。サポートサーバーにだけギルドコマンドとして登録する。

use chrono::Utc;

use crate::{
    data::{Context, Data},
    db,
    error::BotError,
    record::snowflake,
    ticket::{self, Status},
};

pub fn all() -> Vec<poise::Command<Data, BotError>> {
    vec![ticket(), remind()]
}

/// スタッフのロールを持っているか。持っていなければ on_error が「スタッフだけ」と返す
async fn staff_only(ctx: Context<'_>) -> Result<bool, BotError> {
    let Some(member) = ctx.author_member().await else {
        return Ok(false);
    };
    Ok(ctx.data().is_staff(&member.roles))
}

/// 実行したスレッドがチケットとして記録されているか確かめる
async fn current_ticket(ctx: Context<'_>) -> Result<i64, BotError> {
    let thread_id = snowflake(ctx.channel_id());
    if db::thread_exists(&ctx.data().pool, thread_id).await? {
        Ok(thread_id)
    } else {
        Err(BotError::user(
            "このコマンドは、記録対象のフォーラムの投稿 (スレッド) の中で実行してください",
        ))
    }
}

/// チケット (フォーラムの投稿) の操作
#[poise::command(slash_command, guild_only, subcommands("status"))]
pub async fn ticket(_ctx: Context<'_>) -> Result<(), BotError> {
    Ok(())
}

/// このチケットの状態を変えます
#[poise::command(slash_command, guild_only, check = "staff_only")]
pub async fn status(
    ctx: Context<'_>,
    #[description = "新しい状態"] status: Status,
) -> Result<(), BotError> {
    let thread_id = current_ticket(ctx).await?;
    let transition = db::set_status(&ctx.data().pool, thread_id, Some(status))
        .await?
        .ok_or_else(|| BotError::user("このチケットは見つかりませんでした"))?;
    if transition.resolved_changed() {
        ticket::sync_tags(ctx.serenity_context(), ctx.channel_id(), transition.after).await;
    }
    tracing::info!(
        thread_id,
        before = transition.before.as_db(),
        after = transition.after.as_db(),
        "ticket status changed by command"
    );
    let reply = poise::CreateReply::default()
        .content(format!(
            "状態を「{}」から「{}」にしました",
            transition.before.label(),
            transition.after.label()
        ))
        .ephemeral(true);
    ctx.send(reply).await?;
    Ok(())
}

/// 指定した時間のあとに、スタッフ用チャンネルでこのチケットを知らせます
#[poise::command(slash_command, guild_only, check = "staff_only")]
pub async fn remind(
    ctx: Context<'_>,
    #[description = "いつ知らせるか (例: 30m / 12h / 3d / 1w)"] after: String,
    #[description = "知らせるときに添えるメモ"]
    #[max_length = 200]
    note: Option<String>,
) -> Result<(), BotError> {
    let thread_id = current_ticket(ctx).await?;
    let duration = parse_duration(&after).ok_or_else(|| {
        BotError::user("時間は 30m / 12h / 3d / 1w のように、数字と単位 (m / h / d / w) で指定してください (最大 90 日)")
    })?;
    let remind_at = Utc::now() + duration;
    db::insert_reminder(
        &ctx.data().pool,
        thread_id,
        remind_at,
        snowflake(ctx.author().id),
        note.as_deref().map(str::trim).filter(|n| !n.is_empty()),
    )
    .await?;
    let reply = poise::CreateReply::default()
        .content(format!(
            "<t:{}:f> (<t:{}:R>) にスタッフ用チャンネルで知らせます",
            remind_at.timestamp(),
            remind_at.timestamp()
        ))
        .ephemeral(true);
    ctx.send(reply).await?;
    Ok(())
}

/// リマインドの上限 (うっかり桁を間違えた設定を弾く)
const MAX_REMIND: chrono::Duration = chrono::Duration::days(90);

/// `30m` / `12h` / `3d` / `1w` を期間に直す。単位は 1 つだけ受け付ける
pub fn parse_duration(input: &str) -> Option<chrono::Duration> {
    let input = input.trim().to_ascii_lowercase();
    let unit = input.chars().last()?;
    let amount: i64 = input[..input.len() - unit.len_utf8()].trim().parse().ok()?;
    if amount <= 0 {
        return None;
    }
    let duration = match unit {
        'm' => chrono::Duration::try_minutes(amount)?,
        'h' => chrono::Duration::try_hours(amount)?,
        'd' => chrono::Duration::try_days(amount)?,
        'w' => chrono::Duration::try_weeks(amount)?,
        _ => return None,
    };
    (duration <= MAX_REMIND).then_some(duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("30m"), Some(chrono::Duration::minutes(30)));
        assert_eq!(parse_duration(" 12H "), Some(chrono::Duration::hours(12)));
        assert_eq!(parse_duration("3d"), Some(chrono::Duration::days(3)));
        assert_eq!(parse_duration("1w"), Some(chrono::Duration::weeks(1)));
        assert_eq!(parse_duration("90d"), Some(chrono::Duration::days(90)));
    }

    #[test]
    fn rejects_invalid_durations() {
        for input in [
            "",
            "d",
            "0d",
            "-1d",
            "3",
            "3x",
            "1d12h",
            "91d",
            "13w",
            "１d",
            "99999999999999999w",
        ] {
            assert_eq!(parse_duration(input), None, "{input}");
        }
    }
}
