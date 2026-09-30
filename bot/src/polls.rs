//! 日程調整の永続ボタン。Gatewayからの本人・ギルド・投稿IDを検証して回答し、投稿の集計を更新する。
use crate::{
    data::Data,
    error::BotError,
    poll_votes::{self, VoteOutcome},
    tasks::polls::sync_one,
};
use poise::serenity_prelude::{
    self as serenity, ComponentInteraction, CreateInteractionResponse,
    CreateInteractionResponseFollowup,
};

pub fn custom_id(poll: i32, option: i32, answer: &str) -> String {
    format!("poll:{poll}:{option}:{answer}")
}
pub fn parse_custom_id(value: &str) -> Option<(i32, i32, &str)> {
    let mut parts = value.split(':');
    if parts.next() != Some("poll") {
        return None;
    }
    let poll = parts.next()?.parse::<i32>().ok().filter(|id| *id > 0)?;
    let option = parts.next()?.parse::<i32>().ok().filter(|id| *id > 0)?;
    let answer = parts.next()?;
    if parts.next().is_some() || !matches!(answer, "yes" | "maybe" | "no") {
        return None;
    }
    Some((poll, option, answer))
}
pub async fn handle(
    ctx: &serenity::Context,
    data: &Data,
    interaction: &ComponentInteraction,
) -> Result<(), BotError> {
    // 投稿を更新する形でACKし、成功時はチャンネルに何も出さない (押すたびのephemeralは邪魔になる)。
    interaction
        .create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
        .await?;
    let result = record(data, interaction).await;
    let text = match &result {
        Ok(VoteOutcome::Saved) => {
            // 押した投稿の集計をすぐ反映する。他で同期中・失敗時は定期同期に任せる。
            if let Some((poll, _, _)) = parse_custom_id(&interaction.data.custom_id)
                && let Err(error) = sync_one(&ctx.http, &data.pool, &data.site_base_url, poll).await
            {
                tracing::warn!(poll_id = poll, %error, "poll message refresh after vote failed");
            }
            return Ok(());
        }
        Ok(VoteOutcome::Closed) => "投票は締め切られています。Webで結果を確認してください。",
        Ok(VoteOutcome::NotFound) => {
            "日程調整または候補が変更・削除されています。最新の投稿かWebをご確認ください。"
        }
        Ok(VoteOutcome::InvalidAnswer) => {
            "この投票ボタンは使えません。最新の投稿をご確認ください。"
        }
        Err(_) => "回答を保存できませんでした。少し待って再度お試しください。",
    };
    // 保存できなかったときだけ、本人にだけ見える形で知らせる。
    interaction
        .create_followup(
            &ctx.http,
            CreateInteractionResponseFollowup::new()
                .ephemeral(true)
                .content(text),
        )
        .await?;
    result?;
    Ok(())
}
async fn record(data: &Data, interaction: &ComponentInteraction) -> Result<VoteOutcome, BotError> {
    let Some((poll, option, answer)) = parse_custom_id(&interaction.data.custom_id) else {
        return Ok(VoteOutcome::InvalidAnswer);
    };
    let Some(guild) = interaction.guild_id else {
        return Ok(VoteOutcome::InvalidAnswer);
    };
    // guild_idとmemberはDiscord Gatewayが付ける。DM・転送・別投稿からの回答を受け付けない。
    if interaction
        .member
        .as_ref()
        .is_none_or(|member| member.user.id != interaction.user.id)
    {
        return Ok(VoteOutcome::InvalidAnswer);
    }
    let known:bool=sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM schedule_poll_posts s JOIN schedule_polls p ON p.id=s.poll_id JOIN guilds g ON g.guild_id=p.guild_id WHERE p.id=$1 AND p.guild_id=$2 AND s.channel_id=$3 AND s.message_id=$4)")
        .bind(poll).bind(guild.to_string()).bind(interaction.channel_id.to_string()).bind(interaction.message.id.to_string()).fetch_one(&data.pool).await?;
    if !known {
        return Ok(VoteOutcome::NotFound);
    }
    Ok(poll_votes::vote(
        &data.pool,
        &guild.to_string(),
        poll,
        option,
        &interaction.user.id.to_string(),
        answer,
    )
    .await?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn button_ids_are_bounded_and_strict() {
        let value = custom_id(i32::MAX, i32::MAX, "maybe");
        assert!(value.len() <= 100);
        assert_eq!(parse_custom_id(&value), Some((i32::MAX, i32::MAX, "maybe")));
        for value in [
            "other:1:2:yes",
            "poll:0:1:yes",
            "poll:1:2:bad",
            "poll:1:2:yes:extra",
            "poll:1:999999999999999:yes",
        ] {
            assert_eq!(parse_custom_id(value), None);
        }
    }
    #[sqlx::test(migrations = "../api/migrations")]
    async fn accepts_only_members_on_the_registered_guild_message(pool: sqlx::PgPool) {
        use std::{
            collections::HashMap,
            sync::{Arc, atomic::AtomicBool},
        };
        sqlx::query("INSERT INTO guilds(guild_id,name) VALUES ('1','test')")
            .execute(&pool)
            .await
            .unwrap();
        let poll:i32=sqlx::query_scalar("INSERT INTO schedule_polls(guild_id,title,created_by,created_at) VALUES ('1','調整','3',now()) RETURNING id").fetch_one(&pool).await.unwrap();
        let option:i32=sqlx::query_scalar("INSERT INTO schedule_poll_options(poll_id,start_at,end_at,is_all_day,position) VALUES ($1,'2099-01-01','2099-01-01',true,0) RETURNING id").bind(poll).fetch_one(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO schedule_poll_posts(poll_id,channel_id,message_id) VALUES ($1,'2','4')",
        )
        .bind(poll)
        .execute(&pool)
        .await
        .unwrap();
        let data = Data {
            pool: pool.clone(),
            site_base_url: "https://example.com".into(),
            log_channel_id: None,
            invite_url: String::new(),
            support_guild_id: None,
            guild_sync: Arc::new(tokio::sync::Mutex::new(())),
            tasks_started: Arc::new(AtomicBool::new(false)),
            presence_tasks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        };
        let mut member = serenity::Member::default();
        member.user.id = serenity::UserId::new(3);
        member.guild_id = serenity::GuildId::new(1);
        let mut message = serenity::Message::default();
        message.id = serenity::MessageId::new(4);
        message.channel_id = serenity::ChannelId::new(2);
        let mut interaction:ComponentInteraction=serde_json::from_value(serde_json::json!({
            "id":"5","application_id":"6","data":{"custom_id":custom_id(poll,option,"yes"),"component_type":2},"guild_id":"1","channel_id":"2","member":member,"token":"test-token","version":1,"message":message,"locale":"ja","entitlements":[],"attachment_size_limit":10485760
        })).unwrap();
        assert_eq!(
            record(&data, &interaction).await.unwrap(),
            VoteOutcome::Saved
        );
        interaction.message.id = serenity::MessageId::new(7);
        assert_eq!(
            record(&data, &interaction).await.unwrap(),
            VoteOutcome::NotFound
        );
        interaction.message.id = serenity::MessageId::new(4);
        interaction.guild_id = Some(serenity::GuildId::new(8));
        assert_eq!(
            record(&data, &interaction).await.unwrap(),
            VoteOutcome::NotFound
        );
        interaction.guild_id = None;
        assert_eq!(
            record(&data, &interaction).await.unwrap(),
            VoteOutcome::InvalidAnswer
        );
        interaction.guild_id = Some(serenity::GuildId::new(1));
        interaction.member = None;
        assert_eq!(
            record(&data, &interaction).await.unwrap(),
            VoteOutcome::InvalidAnswer
        );
    }
}
