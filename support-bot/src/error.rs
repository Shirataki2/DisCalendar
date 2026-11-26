use poise::serenity_prelude as serenity;

use crate::data::{Context, Data};

/// コマンド・イベントハンドラが返すエラー
#[derive(Debug, thiserror::Error)]
pub enum BotError {
    // serenity::Error は大きい (clippy::result_large_err) ので Box に入れる
    #[error("Discord error: {0}")]
    Serenity(#[source] Box<serenity::Error>),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// 入力や権限などユーザー側の問題。本人にだけ (ephemeral) 返し、ログには出さない
    #[error("{0}")]
    User(String),
}

impl From<serenity::Error> for BotError {
    fn from(e: serenity::Error) -> Self {
        Self::Serenity(Box::new(e))
    }
}

impl BotError {
    pub fn user(message: impl Into<String>) -> Self {
        Self::User(message.into())
    }
}

const UNEXPECTED_ERROR: &str = "予期せぬエラーが発生しました。時間をおいて再度お試しください";

/// poise からのエラー通知。詳細はログ (と Sentry) にだけ出し、実行した人には短く返す。
/// サポートサーバー専用でスタッフしか使わないので、メッセージは日本語だけにしている
pub async fn on_error(error: poise::FrameworkError<'_, Data, BotError>) {
    use poise::FrameworkError;
    match error {
        FrameworkError::Setup { error, .. } => {
            tracing::error!(%error, "failed to set up the support bot");
        }
        FrameworkError::EventHandler { error, event, .. } => {
            tracing::error!(%error, event = event.snake_case_name(), "error in event handler");
        }
        FrameworkError::Command {
            error: BotError::User(message),
            ctx,
            ..
        } => reply_ephemeral(ctx, message).await,
        FrameworkError::Command { error, ctx, .. } => {
            tracing::error!(%error, command = ctx.command().name, "error in command");
            reply_ephemeral(ctx, UNEXPECTED_ERROR).await;
        }
        FrameworkError::CommandCheckFailed { error, ctx, .. } => {
            if let Some(error) = error {
                tracing::error!(%error, command = ctx.command().name, "error in command check");
                reply_ephemeral(ctx, UNEXPECTED_ERROR).await;
            } else {
                reply_ephemeral(ctx, "このコマンドはスタッフだけが使えます").await;
            }
        }
        FrameworkError::GuildOnly { ctx, .. } => {
            reply_ephemeral(ctx, "このコマンドはサーバー内でのみ実行できます").await;
        }
        other => {
            if let Some(ctx) = other.ctx() {
                reply_ephemeral(ctx, UNEXPECTED_ERROR).await;
            }
            tracing::warn!(error = %other, "unhandled framework error");
        }
    }
}

async fn reply_ephemeral(ctx: Context<'_>, content: impl Into<String>) {
    let reply = poise::CreateReply::default()
        .content(content)
        .allowed_mentions(serenity::CreateAllowedMentions::new())
        .ephemeral(true);
    if let Err(e) = ctx.send(reply).await {
        tracing::warn!(error = %e, command = ctx.command().name, "failed to send error reply");
    }
}
