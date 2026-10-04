use poise::serenity_prelude as serenity;

use crate::data::{Context, Data};

/// コマンドの panic を記録するときのログ target (#17)。
/// panic 自体はスタックトレース付きで Sentry の panic integration が送るので、poise が
/// `catch_unwind` で拾ったあとのこのログまでイベントにすると同じ障害が二重に届く。
/// main.rs の event_filter がこの target をパンくず扱いにして、送信を 1 回に保つ
pub const COMMAND_PANIC_LOG_TARGET: &str = "discalendar_bot::command_panic";

/// コマンド・イベントハンドラが返すエラー
#[derive(Debug, thiserror::Error)]
pub enum BotError {
    // serenity::Error は大きい (clippy::result_large_err) ので Box に入れる
    #[error("Discord error: {0}")]
    Serenity(#[source] Box<serenity::Error>),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("recurrence error: {0}")]
    Recurrence(#[from] anyhow::Error),
    /// 入力や権限などユーザー側の問題。メッセージをそのまま本人にだけ (ephemeral) 返し、ログには出さない
    #[error("{0}")]
    User(UserMessage),
}

impl From<serenity::Error> for BotError {
    fn from(e: serenity::Error) -> Self {
        Self::Serenity(Box::new(e))
    }
}

impl BotError {
    pub fn user(message: impl Into<String>) -> Self {
        Self::User(UserMessage {
            ja: message.into(),
            en: None,
        })
    }
}

/// 予期しないエラーのときにユーザーへ返すメッセージ (詳細はログにだけ出す)
const UNEXPECTED_ERROR: &str = "予期せぬエラーが発生しました。時間をおいて再度お試しください";

/// poise からのエラー通知。詳細を記録し、利用者の言語で短い案内を返す。
pub async fn on_error(error: poise::FrameworkError<'_, Data, BotError>) {
    let locale = error
        .ctx()
        .map(crate::i18n::user_locale)
        .unwrap_or_default();
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
        } => reply_ephemeral(ctx, message.localized(locale)).await,
        FrameworkError::Command { error, ctx, .. } => {
            tracing::error!(%error, command = ctx.command().name, "error in command");
            reply_ephemeral(ctx, locale.text(UNEXPECTED_ERROR, "An unexpected error occurred. Please try again later.")).await;
        }
        FrameworkError::CommandPanic { ctx, payload, .. } => {
            tracing::error!(
                target: COMMAND_PANIC_LOG_TARGET,
                ?payload,
                command = ctx.command().name,
                "command panicked"
            );
            reply_ephemeral(ctx, locale.text(UNEXPECTED_ERROR, "An unexpected error occurred. Please try again later.")).await;
        }
        // check 関数が false を返したとき (error: None) は check 側で理由を返信済み
        FrameworkError::CommandCheckFailed {
            error: Some(error),
            ctx,
            ..
        } => {
            tracing::error!(%error, command = ctx.command().name, "error in command check");
            reply_ephemeral(ctx, locale.text(UNEXPECTED_ERROR, "An unexpected error occurred. Please try again later.")).await;
        }
        FrameworkError::CommandCheckFailed { error: None, .. } => {}
        FrameworkError::GuildOnly { ctx, .. } => {
            reply_ephemeral(ctx, locale.text("このコマンドはサーバー内でのみ実行できます", "This command can only be used in a server.")).await;
        }
        FrameworkError::NotAnOwner { ctx, .. } => {
            reply_ephemeral(ctx, locale.text("このコマンドは Bot のオーナーのみ実行できます", "Only the bot owner can use this command.")).await;
        }
        FrameworkError::ArgumentParse {
            ctx, input, ..
        } => {
            let message = match input {
                Some(input) => crate::tr!(locale, "引数 `{input}` を解釈できませんでした。入力形式を確認してください", "Could not parse `{input}`. Please check the input format."),
                None => locale.text("引数を解釈できませんでした。入力形式を確認してください", "Could not parse the arguments. Please check the input format.").into(),
            };
            reply_ephemeral(ctx, message).await;
        }
        FrameworkError::CooldownHit { ctx, remaining_cooldown, .. } => {
            let seconds = remaining_cooldown.as_secs() + 1;
            reply_ephemeral(ctx, crate::tr!(locale, "あと {seconds} 秒待ってから実行してください", "Please wait {seconds} seconds before trying again.")).await;
        }
        FrameworkError::MissingBotPermissions { ctx, .. } => reply_ephemeral(ctx, locale.text("Bot の権限が不足しています。管理者に確認してください", "The bot is missing required permissions. Please ask a server administrator.")).await,
        FrameworkError::MissingUserPermissions { ctx, .. } => reply_ephemeral(ctx, locale.text("このコマンドを実行する権限がありません", "You do not have permission to use this command.")).await,
        FrameworkError::DmOnly { ctx, .. } => reply_ephemeral(ctx, locale.text("このコマンドは DM でのみ実行できます", "This command can only be used in direct messages.")).await,
        FrameworkError::NsfwOnly { ctx, .. } => reply_ephemeral(ctx, locale.text("このコマンドは年齢制限付きチャンネルでのみ実行できます", "This command requires an age-restricted channel.")).await,
        FrameworkError::SubcommandRequired { ctx, .. } => reply_ephemeral(ctx, locale.text("サブコマンドを選択してください", "Please select a subcommand.")).await,
        FrameworkError::CommandStructureMismatch { ctx, .. } => reply_ephemeral(Context::Application(ctx), locale.text("コマンドの定義が更新されています。再登録後にお試しください", "The command definition has changed. Please try again after the commands have been registered again.")).await,
        FrameworkError::UnknownInteraction { ctx, interaction, .. } => {
            let locale = crate::i18n::Locale::resolve(&interaction.locale);
            let reply = serenity::CreateInteractionResponse::Message(serenity::CreateInteractionResponseMessage::new().ephemeral(true).content(locale.text("コマンドの定義が更新されています。再登録後にお試しください", "The command definition has changed. Please try again after the commands have been registered again.")).allowed_mentions(serenity::CreateAllowedMentions::new()));
            if let Err(error) = interaction.create_response(&ctx.http, reply).await {
                tracing::warn!(%error, "failed to reply to unknown interaction");
            }
        }
        other => {
            if let Some(ctx) = other.ctx() {
                reply_ephemeral(ctx, locale.text(UNEXPECTED_ERROR, "An unexpected error occurred. Please try again later.")).await;
            }
            tracing::warn!("unhandled framework error");
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

/// 入力検証時は両言語を保持し、返信時に Discord の利用者言語を選ぶ。
#[derive(Debug)]
pub struct UserMessage {
    pub ja: String,
    pub en: Option<String>,
}
impl std::fmt::Display for UserMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.ja)
    }
}
impl UserMessage {
    pub fn localized(self, locale: crate::i18n::Locale) -> String {
        if locale == crate::i18n::Locale::En {
            self.en.filter(|text| !text.is_empty()).unwrap_or(self.ja)
        } else {
            self.ja
        }
    }
}
#[macro_export]
macro_rules! user_error {
    ($ja:literal, $en:literal $(, $args:expr)* $(,)?) => {
        $crate::error::BotError::User($crate::error::UserMessage {
            ja: format!($ja $(, $args)*), en: Some(format!($en $(, $args)*)),
        })
    };
}
