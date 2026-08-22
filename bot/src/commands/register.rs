use crate::{data::Context, error::BotError};

/// スラッシュコマンドを Discord に登録・削除します (Bot のオーナー専用)。
///
/// スラッシュコマンドとしては登録しない (登録前に呼べる必要がある) ので、
/// Bot をメンションして `@DisCalendar register` のように呼ぶ。
/// 押したボタンに応じて、このサーバーだけ / 全サーバー (グローバル) に登録・削除する。
/// グローバル登録は反映に時間がかかることがあるので、動作確認はサーバー登録で行う
/// (両方に登録すると同じコマンドが 2 つ並ぶので、確認が終わったらサーバー側を削除する)
#[poise::command(prefix_command, owners_only, hide_in_help)]
pub async fn register(ctx: Context<'_>) -> Result<(), BotError> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}
