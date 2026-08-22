//! 埋め込みのフィールドをページ送りするボタン付きメッセージ (旧 `utils/paginator.rs` 相当)。
//!
//! `/list` の予定一覧で使う。「前へ」「次へ」でページを切り替え、「完了」でメッセージを消す。
//! 操作がないまま一定時間たつとボタンを外す

use std::time::Duration;

use poise::serenity_prelude::{self as serenity, CreateActionRow, CreateButton, CreateEmbed};

use crate::{data::Context, error::BotError};

/// 操作がないときにボタンを外すまでの時間
const IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone)]
pub struct Paginator {
    per_page: usize,
    template: CreateEmbed,
    fields: Vec<(String, String, bool)>,
}

impl Paginator {
    /// `template` はタイトルや色など全ページ共通の埋め込み。フィールドとフッターはページごとに付ける
    pub fn new(per_page: usize, template: CreateEmbed) -> Self {
        assert!(per_page > 0, "per_page must be positive");
        Self {
            per_page,
            template,
            fields: Vec::new(),
        }
    }

    pub fn add(&mut self, name: impl Into<String>, value: impl Into<String>, inline: bool) {
        self.fields.push((name.into(), value.into(), inline));
    }

    pub fn total_pages(&self) -> usize {
        self.fields.len().div_ceil(self.per_page).max(1)
    }

    /// `page` は 0 始まり
    fn embed(&self, page: usize) -> CreateEmbed {
        let fields = self
            .fields
            .iter()
            .skip(page * self.per_page)
            .take(self.per_page)
            .cloned();
        self.template
            .clone()
            .fields(fields)
            .footer(serenity::CreateEmbedFooter::new(format!(
                "Page {}/{}",
                page + 1,
                self.total_pages()
            )))
    }

    fn components(&self, ids: &ButtonIds, page: usize, disabled: bool) -> Vec<CreateActionRow> {
        vec![CreateActionRow::Buttons(vec![
            CreateButton::new(&ids.prev)
                .style(serenity::ButtonStyle::Primary)
                .label("前へ")
                .disabled(disabled || page == 0),
            CreateButton::new(&ids.finish)
                .style(serenity::ButtonStyle::Danger)
                .label("完了")
                .disabled(disabled),
            CreateButton::new(&ids.next)
                .style(serenity::ButtonStyle::Primary)
                .label("次へ")
                .disabled(disabled || page + 1 >= self.total_pages()),
        ])]
    }

    /// メッセージを送り、ボタン操作に応答し続ける。「完了」か無操作のタイムアウトで戻る
    pub async fn start(self, ctx: Context<'_>) -> Result<(), BotError> {
        // 他のコマンド・他のユーザーのボタンと区別できるよう、この呼び出し固有の ID を付ける
        let ids = ButtonIds::new(ctx.id());
        let mut page = 0;

        let reply = ctx
            .send(
                poise::CreateReply::default()
                    .embed(self.embed(page))
                    .components(self.components(&ids, page, false)),
            )
            .await?;
        // ボタンを外すときの編集に使う。スラッシュコマンドの interaction トークン (15 分) が切れても
        // 自分のメッセージなので通常の編集 API で書き換えられる
        let mut message = reply.into_message().await?;

        loop {
            let press = serenity::ComponentInteractionCollector::new(ctx.serenity_context())
                .author_id(ctx.author().id)
                .channel_id(ctx.channel_id())
                .message_id(message.id)
                .filter({
                    let ids = ids.clone();
                    move |press| ids.contains(&press.data.custom_id)
                })
                .timeout(IDLE_TIMEOUT)
                .await;
            let Some(press) = press else {
                // 無操作タイムアウト: ボタンだけ外して内容は残す
                message
                    .edit(
                        ctx.http(),
                        serenity::EditMessage::new().components(Vec::new()),
                    )
                    .await?;
                return Ok(());
            };

            if press.data.custom_id == ids.finish {
                press
                    .create_response(ctx.http(), serenity::CreateInteractionResponse::Acknowledge)
                    .await?;
                message.delete(ctx.http()).await?;
                return Ok(());
            }
            if press.data.custom_id == ids.prev {
                page = page.saturating_sub(1);
            } else if press.data.custom_id == ids.next {
                page = (page + 1).min(self.total_pages() - 1);
            }
            press
                .create_response(
                    ctx.http(),
                    serenity::CreateInteractionResponse::UpdateMessage(
                        serenity::CreateInteractionResponseMessage::new()
                            .embed(self.embed(page))
                            .components(self.components(&ids, page, false)),
                    ),
                )
                .await?;
        }
    }
}

#[derive(Debug, Clone)]
struct ButtonIds {
    prev: String,
    finish: String,
    next: String,
}

impl ButtonIds {
    fn new(invocation_id: u64) -> Self {
        Self {
            prev: format!("{invocation_id}:prev"),
            finish: format!("{invocation_id}:finish"),
            next: format!("{invocation_id}:next"),
        }
    }

    fn contains(&self, id: &str) -> bool {
        id == self.prev || id == self.finish || id == self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paginator(items: usize) -> Paginator {
        let mut p = Paginator::new(4, CreateEmbed::new().title("t"));
        for i in 0..items {
            p.add(format!("name{i}"), format!("value{i}"), false);
        }
        p
    }

    #[test]
    fn total_pages_rounds_up_and_is_at_least_one() {
        assert_eq!(paginator(0).total_pages(), 1);
        assert_eq!(paginator(1).total_pages(), 1);
        assert_eq!(paginator(4).total_pages(), 1);
        assert_eq!(paginator(5).total_pages(), 2);
        assert_eq!(paginator(9).total_pages(), 3);
    }

    #[test]
    fn button_ids_are_scoped_to_the_invocation() {
        let ids = ButtonIds::new(42);
        assert!(ids.contains("42:prev"));
        assert!(ids.contains("42:finish"));
        assert!(ids.contains("42:next"));
        assert!(!ids.contains("43:next"));
        assert!(!ids.contains("42:other"));
    }
}
