//! Gateway イベントの処理 (旧 `event.rs` 相当)。
//!
//! `guilds` テーブルは web のサーバー選択が「Bot 参加済み」の判定に使うので、
//! Bot の参加・退出・ギルド名やアイコンの変更をここで反映する。
//! 定期タスク (通知 / presence / アイコン更新) は `ShardsReady` で起動する。
//! コマンドの実行ログ (旧 `pre_command`) は `commands::log_invocation`。

use std::collections::HashSet;

use poise::serenity_prelude::{self as serenity, FullEvent};

use crate::{data::Data, error::BotError, models::guilds, tasks};

pub async fn handle_event(
    ctx: &serenity::Context,
    event: &FullEvent,
    _framework: poise::FrameworkContext<'_, Data, BotError>,
    data: &Data,
) -> Result<(), BotError> {
    match event {
        FullEvent::Ready { data_about_bot } => {
            tracing::info!(
                shard = ctx.shard_id.0,
                user = %data_about_bot.user.name,
                guilds = data_about_bot.guilds.len(),
                "shard ready"
            );
            // 停止中にサーバーから退出させられた分は GuildDelete が届かないので、ここで掃除する
            reconcile_guilds(ctx, data).await?;
        }
        // 全シャードが Ready を受け取った後に一度だけ発火する (autosharded 起動でも定期タスクは1回だけ起動したい)。
        // 念のため `mark_tasks_started` でも多重起動を防ぐ
        FullEvent::ShardsReady { total_shards } => {
            if data.mark_tasks_started() {
                tracing::info!(total_shards, "starting periodic tasks");
                tasks::spawn_all(ctx.clone(), data.clone());
            }
        }
        // 起動時に参加済みの各ギルドと、新しく参加したときに届く。
        // 起動時の分も upsert して、停止中に変わった名前・アイコンや取りこぼしを取り戻す
        FullEvent::GuildCreate { guild, is_new } => {
            let guild_id = guild.id.to_string();
            if !upsert_from_cache(ctx, data, guild.id).await? {
                tracing::debug!(
                    guild_id,
                    "already left before handling GuildCreate, skipping"
                );
                return Ok(());
            }
            // is_new は「Ready の時点で参加していなかったギルド」= 新規参加 (cache 機能が必要)
            if *is_new == Some(true) {
                tracing::info!(guild_id, name = %guild.name, "joined a guild");
                notify_log_channel(
                    ctx,
                    data,
                    "DisCalendar - New Guild",
                    0x0000ff,
                    format!("{} ({})", guild.name, guild_id),
                )
                .await;
            } else {
                tracing::debug!(guild_id, name = %guild.name, "registered guild");
            }
        }
        FullEvent::GuildUpdate { new_data, .. } => {
            let guild_id = new_data.id.to_string();
            if !upsert_from_cache(ctx, data, new_data.id).await? {
                tracing::debug!(
                    guild_id,
                    "already left before handling GuildUpdate, skipping"
                );
                return Ok(());
            }
            tracing::info!(guild_id, name = %new_data.name, "updated guild");
        }
        FullEvent::GuildDelete { incomplete, full } => {
            let guild_id = incomplete.id.to_string();
            // unavailable = true は Discord 側の障害でギルドが一時的に落ちただけ (退出ではない)
            if incomplete.unavailable {
                tracing::warn!(
                    guild_id,
                    "guild became unavailable (outage), keeping the row"
                );
                return Ok(());
            }
            let deleted = {
                // upsert や reconcile と交錯しないよう直列化する
                let _guard = data.guild_sync.lock().await;
                // 退出直後に再参加していれば、その GuildCreate がキャッシュに先に反映されている (登録し直される) ので消さない
                if ctx.cache.guild(incomplete.id).is_some() {
                    tracing::debug!(
                        guild_id,
                        "rejoined before handling GuildDelete, keeping the row"
                    );
                    return Ok(());
                }
                guilds::delete(&data.pool, &guild_id).await?
            };
            let name = full
                .as_ref()
                .map_or_else(|| "[unknown guild]".to_owned(), |g| g.name.clone());
            tracing::info!(guild_id, name = %name, deleted, "left a guild");
            notify_log_channel(
                ctx,
                data,
                "DisCalendar - Leave Guild",
                0x00ff00,
                format!("{name} ({guild_id})"),
            )
            .await;
        }
        _ => {}
    }
    Ok(())
}

/// DB に残っているが Bot が参加していないギルドの行を消す。
/// Bot の停止中 (や再接続の間) にサーバーから退出させられると `GuildDelete` が届かず行が残り、
/// web のサーバー選択では「参加済み」に見えるのに API はメンバー確認で拒否する状態になるため、
/// `Ready` のたびに `GET /users/@me/guilds` で現在の参加一覧を取り直して突き合わせる
/// (キャッシュの一覧はシャードの接続順によって全件揃う前に `CacheReady` が出ることがあるので使わない)。
async fn reconcile_guilds(ctx: &serenity::Context, data: &Data) -> Result<(), BotError> {
    // DB の一覧取得 → Discord の一覧取得 → 削除 の間に参加し直したギルドの行 (GuildCreate の upsert) を
    // 消してしまわないよう、この間は upsert 側を待たせる (待たされた upsert は削除の後に実行されて行が戻る)
    let _guard = data.guild_sync.lock().await;
    let known = guilds::list_ids(&data.pool).await?;
    if known.is_empty() {
        return Ok(());
    }
    let current = fetch_joined_guild_ids(&ctx.http).await?;
    let stale: Vec<String> = known
        .into_iter()
        .filter(|id| !current.contains(id))
        .collect();
    if stale.is_empty() {
        tracing::debug!(joined = current.len(), "guilds table is in sync");
        return Ok(());
    }
    let deleted = guilds::delete_many(&data.pool, &stale).await?;
    tracing::info!(deleted, stale = ?stale, "removed guilds the bot left while offline");
    Ok(())
}

/// Bot が参加しているギルドの ID を全件取る。
/// `GET /users/@me/guilds` は 1 回 200 件までで、カーソルなしの先頭ページがどちらの端 (新しい側) から返るかに
/// 依存しないよう、最小の Snowflake を `after` にして古い方から昇順に辿り、各ページの最大 ID を次の `after` にする
/// (200 件を超えるときに取りこぼすと、reconcile で参加中のギルドを誤って消してしまう)
async fn fetch_joined_guild_ids(http: &serenity::Http) -> Result<HashSet<String>, BotError> {
    const PAGE_SIZE: u64 = 200;
    let mut ids = HashSet::new();
    let mut after: u64 = 1;
    loop {
        let page = http
            .get_guilds(
                Some(serenity::GuildPagination::After(serenity::GuildId::new(
                    after,
                ))),
                Some(PAGE_SIZE),
            )
            .await?;
        let is_last_page = (page.len() as u64) < PAGE_SIZE;
        let Some(max_id) = page.iter().map(|g| g.id.get()).max() else {
            return Ok(ids);
        };
        ids.extend(page.into_iter().map(|g| g.id.to_string()));
        // 同じページが返り続けるような想定外の応答で無限ループしないための保険
        if is_last_page || max_id <= after {
            return Ok(ids);
        }
        after = max_id;
    }
}

/// キャッシュ上の最新の名前・アイコンで `guilds` を upsert する。登録したら true、既に退出済み (キャッシュに無い) なら false。
///
/// イベントハンドラは並行に走るので、イベントの payload をそのまま書くと完了順がゲートウェイのイベント順と
/// 逆転したときに古い名前・アイコンが残る。serenity はキャッシュをイベント順に同期更新してからハンドラを
/// 起動するので、ロックを取ってからキャッシュの値を書けば、最後に実行された upsert が常に最新の値になる。
/// キャッシュの参照 (内部ロック) は await をまたいで保持しないよう、値をコピーしてすぐ捨てる
async fn upsert_from_cache(
    ctx: &serenity::Context,
    data: &Data,
    guild_id: serenity::GuildId,
) -> Result<bool, BotError> {
    let _guard = data.guild_sync.lock().await;
    let Some((name, icon_url)) = ctx
        .cache
        .guild(guild_id)
        .map(|g| (g.name.clone(), g.icon_url()))
    else {
        return Ok(false);
    };
    guilds::upsert(
        &data.pool,
        &guild_id.to_string(),
        &name,
        icon_url.as_deref(),
    )
    .await?;
    Ok(true)
}

/// 参加・退出を `BOT_LOG_CHANNEL_ID` のチャンネルに埋め込みで通知する (未設定なら何もしない)。
/// 通知の失敗で DB 更新まで失敗扱いにしないよう、エラーはログに出すだけにする
async fn notify_log_channel(
    ctx: &serenity::Context,
    data: &Data,
    title: &str,
    colour: u32,
    description: String,
) {
    let Some(channel_id) = data.log_channel_id else {
        return;
    };
    let embed = serenity::CreateEmbed::new()
        .title(title)
        .colour(colour)
        .description(description);
    if let Err(e) = channel_id
        .send_message(&ctx.http, serenity::CreateMessage::new().embed(embed))
        .await
    {
        tracing::warn!(error = %e, channel_id = channel_id.get(), "failed to send to log channel");
    }
}
