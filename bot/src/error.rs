use poise::serenity_prelude as serenity;

use crate::data::Data;

/// コマンド・イベントハンドラが返すエラー
#[derive(Debug, thiserror::Error)]
pub enum BotError {
    // serenity::Error は大きい (clippy::result_large_err) ので Box に入れる
    #[error("Discord error: {0}")]
    Serenity(#[source] Box<serenity::Error>),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<serenity::Error> for BotError {
    fn from(e: serenity::Error) -> Self {
        Self::Serenity(Box::new(e))
    }
}

/// poise からのエラー通知。tracing に出し、コマンド実行時のエラーなどは poise の既定処理
/// (ユーザーへの ephemeral な返信) に任せる
pub async fn on_error(error: poise::FrameworkError<'_, Data, BotError>) {
    match &error {
        poise::FrameworkError::Setup { error, .. } => {
            tracing::error!(%error, "failed to set up the bot");
            return;
        }
        poise::FrameworkError::EventHandler { error, event, .. } => {
            tracing::error!(%error, event = event.snake_case_name(), "error in event handler");
            return;
        }
        poise::FrameworkError::Command { error, ctx, .. } => {
            tracing::error!(%error, command = ctx.command().name, "error in command");
        }
        _ => {}
    }
    if let Err(e) = poise::builtins::on_error(error).await {
        tracing::error!(error = %e, "failed to handle framework error");
    }
}
