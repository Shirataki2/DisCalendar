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
    /// 入力や権限などユーザー側の問題。メッセージをそのまま本人にだけ (ephemeral) 返し、ログには出さない
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

/// 予期しないエラーのときにユーザーへ返すメッセージ (詳細はログにだけ出す)
const UNEXPECTED_ERROR: &str = "予期せぬエラーが発生しました。時間をおいて再度お試しください";

/// poise からのエラー通知。tracing に出し、ユーザーへは日本語で短く返す。
/// ここで扱わない種類は poise の既定処理 (英語の ephemeral な返信) に任せる
pub async fn on_error(error: poise::FrameworkError<'_, Data, BotError>) {
    use poise::FrameworkError;
    match error {
        FrameworkError::Setup { error, .. } => {
            tracing::error!(%error, "failed to set up the bot");
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
        FrameworkError::CommandPanic { ctx, payload, .. } => {
            tracing::error!(?payload, command = ctx.command().name, "command panicked");
            reply_ephemeral(ctx, UNEXPECTED_ERROR).await;
        }
        // check 関数が false を返したとき (error: None) は check 側で理由を返信済み
        FrameworkError::CommandCheckFailed {
            error: Some(error),
            ctx,
            ..
        } => {
            tracing::error!(%error, command = ctx.command().name, "error in command check");
            reply_ephemeral(ctx, UNEXPECTED_ERROR).await;
        }
        FrameworkError::CommandCheckFailed { error: None, .. } => {}
        FrameworkError::GuildOnly { ctx, .. } => {
            reply_ephemeral(ctx, "このコマンドはサーバー内でのみ実行できます").await;
        }
        FrameworkError::NotAnOwner { ctx, .. } => {
            reply_ephemeral(ctx, "このコマンドは Bot のオーナーのみ実行できます").await;
        }
        FrameworkError::ArgumentParse {
            ctx, input, error, ..
        } => {
            let message = match input {
                Some(input) => format!("引数 `{input}` を解釈できませんでした: {error}"),
                None => format!("引数を解釈できませんでした: {error}"),
            };
            reply_ephemeral(ctx, message).await;
        }
        other => {
            if let Err(e) = poise::builtins::on_error(other).await {
                tracing::error!(error = %e, "failed to handle framework error");
            }
        }
    }
}

/// 本人にだけ見えるメッセージで返す。返信に失敗しても (元のエラー処理中なので) ログに出すだけにする
async fn reply_ephemeral(ctx: Context<'_>, content: impl Into<String>) {
    let reply = poise::CreateReply::default()
        .content(content)
        .allowed_mentions(serenity::CreateAllowedMentions::new())
        .ephemeral(true);
    if let Err(e) = ctx.send(reply).await {
        tracing::warn!(error = %e, command = ctx.command().name, "failed to send error reply");
    }
}
