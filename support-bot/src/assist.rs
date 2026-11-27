//! 不具合対応の補助 (#426): `/ticket assist` のパネルから使う、Claude による要約・分類・返信案と GitHub Issue の起票。
//!
//! 決まり:
//! - 結果はすべて押したスタッフ本人にだけ見えるメッセージ (ephemeral) で返す。Bot はチケットに返信しない
//! - Claude API に送るのはスタッフがボタンを押したときだけ。送った記録 (メタ情報のみ) を `support_ai_requests` に残す
//! - Issue は自動では起票しない。下書きを伏せ字にしてから見せ、スタッフがモーダルで確認・修正して送信したときだけ起票する
//! - ephemeral のボタンでも custom_id は偽装できる前提で、ボタン・モーダルの送信ごとにスタッフのロールとチケットを確かめる

use std::time::{Duration, Instant};

use chrono::Utc;
use poise::serenity_prelude::{
    self as serenity, ActionRowComponent, ButtonStyle, ComponentInteraction, CreateActionRow,
    CreateAllowedMentions, CreateButton, CreateEmbed, CreateEmbedFooter, CreateInputText,
    CreateInteractionResponse, CreateInteractionResponseMessage, CreateModal,
    EditInteractionResponse, InputTextStyle, Interaction, Member, ModalInteraction,
};

use crate::{
    claude::{self, Claude, Outcome, Role, TicketInput, TicketMessage},
    data::Data,
    db,
    error::BotError,
    github,
    mcp::store,
    record::snowflake,
    ticket::Status,
};

/// このモジュールが扱う custom_id の接頭辞
const PREFIX: &str = "support:";
const ASSIST_ID: &str = "support:assist";
const DRAFT_ID: &str = "support:issue_draft";
/// 「編集して起票」ボタン。後ろに下書きの番号が付く
const EDIT_PREFIX: &str = "support:issue_edit:";
/// 起票のモーダル。後ろに下書きの番号が付く
const MODAL_PREFIX: &str = "support:issue_modal:";
const TITLE_INPUT: &str = "title";
const BODY_INPUT: &str = "body";

/// GitHub の Issue のタイトルの上限
const MAX_TITLE_CHARS: usize = 256;
/// Discord のモーダルの入力欄の上限
const MAX_BODY_CHARS: usize = 4000;
/// 1 つのメッセージの埋め込みは合計 6000 文字まで。要約と返信案の 2 つ (と見出し・フッタ) で収まるように分ける
const MAX_SUMMARY_CHARS: usize = 2000;
const MAX_REPLY_CHARS: usize = 3600;
/// 下書きを覚えておく時間。過ぎたら作り直してもらう (Bot を再起動しても消える)
const DRAFT_TTL: Duration = Duration::from_secs(30 * 60);
/// 一度に読む発言の上限。これより長いスレッドは、最初の投稿と新しい側の発言だけを読む
const MAX_MESSAGES: i64 = 2_000;

const ISSUE_FOOTER: &str =
    "\n\n---\nサポートのチケットから起票しました (利用者の名前・ID などは伏せています)";

const UNEXPECTED_ERROR: &str = "予期せぬエラーが発生しました。時間をおいて再度お試しください";

/// スタッフの確認を待っている Issue の下書き
#[derive(Debug, Clone)]
pub struct PendingIssue {
    pub thread_id: i64,
    pub title: String,
    pub body: String,
    pub created_at: Instant,
}

/// `/ticket assist` のパネル (本文とボタン)
pub fn panel(data: &Data, github_issue: Option<i32>) -> (String, Vec<CreateActionRow>) {
    let mut lines = vec!["このチケットの対応を手伝います。結果はあなたにだけ表示され、Bot がチケットに返信することはありません。".to_owned()];
    if data.settings.claude.is_none() {
        lines.push("- Claude API が未設定のため、要約・分類・返信案は使えません".into());
    }
    if data.settings.github.is_none() {
        lines.push("- GitHub が未設定のため、Issue 化は使えません".into());
    }
    if let Some(issue) = github_issue {
        lines.push(format!("- このチケットは起票済みです (Issue #{issue})"));
    }
    let buttons = vec![
        CreateButton::new(ASSIST_ID)
            .label("要約・分類・返信案")
            .style(ButtonStyle::Primary)
            .disabled(data.settings.claude.is_none()),
        CreateButton::new(DRAFT_ID)
            .label("Issue の下書き")
            .style(ButtonStyle::Secondary)
            .disabled(data.settings.github.is_none() || github_issue.is_some()),
    ];
    (lines.join("\n"), vec![CreateActionRow::Buttons(buttons)])
}

/// Gateway の InteractionCreate から呼ぶ。このモジュールのボタン・モーダル以外は無視する
pub async fn handle_interaction(
    ctx: &serenity::Context,
    data: &Data,
    interaction: &Interaction,
) -> Result<(), BotError> {
    match interaction {
        Interaction::Component(component) if component.data.custom_id.starts_with(PREFIX) => {
            if !allowed(data, component.guild_id, component.member.as_ref()) {
                return reply_component(ctx, component, "この操作はスタッフだけが使えます").await;
            }
            let result = handle_component(ctx, data, component).await;
            if let Err(error) = &result {
                tracing::error!(%error, custom_id = %component.data.custom_id, "failed to handle an assist button");
                // 応答を保留したあとなら、保留中の表示をエラーに置き換える (まだなら新しく返す)
                let edited = component
                    .edit_response(
                        ctx,
                        EditInteractionResponse::new().content(UNEXPECTED_ERROR),
                    )
                    .await;
                if edited.is_err() {
                    reply_component(ctx, component, UNEXPECTED_ERROR).await.ok();
                }
            }
            Ok(())
        }
        Interaction::Modal(modal) if modal.data.custom_id.starts_with(PREFIX) => {
            if !allowed(data, modal.guild_id, modal.member.as_ref()) {
                let reply = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("この操作はスタッフだけが使えます")
                        .ephemeral(true),
                );
                modal.create_response(ctx, reply).await?;
                return Ok(());
            }
            if let Err(error) = handle_modal(ctx, data, modal).await {
                tracing::error!(%error, "failed to create a GitHub issue from a ticket");
                modal
                    .edit_response(
                        ctx,
                        EditInteractionResponse::new().content(UNEXPECTED_ERROR),
                    )
                    .await
                    .ok();
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn allowed(data: &Data, guild_id: Option<serenity::GuildId>, member: Option<&Member>) -> bool {
    guild_id == Some(data.settings.guild_id) && member.is_some_and(|m| data.is_staff(&m.roles))
}

async fn reply_component(
    ctx: &serenity::Context,
    component: &ComponentInteraction,
    content: &str,
) -> Result<(), BotError> {
    let reply = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(content)
            .allowed_mentions(CreateAllowedMentions::new())
            .ephemeral(true),
    );
    component.create_response(ctx, reply).await?;
    Ok(())
}

async fn edit(
    ctx: &serenity::Context,
    component: &ComponentInteraction,
    response: EditInteractionResponse,
) -> Result<(), BotError> {
    component
        .edit_response(ctx, response.allowed_mentions(CreateAllowedMentions::new()))
        .await?;
    Ok(())
}

/// 押された場所が記録対象のチケット (フォーラムの投稿) か
async fn is_ticket(data: &Data, channel_id: serenity::ChannelId) -> Result<bool, BotError> {
    let parent = db::thread_parent(&data.pool, snowflake(channel_id)).await?;
    Ok(parent.is_some_and(|parent| {
        data.is_record_channel(serenity::ChannelId::new(parent.cast_unsigned()))
    }))
}

async fn handle_component(
    ctx: &serenity::Context,
    data: &Data,
    component: &ComponentInteraction,
) -> Result<(), BotError> {
    let custom_id = component.data.custom_id.as_str();
    if let Some(key) = custom_id.strip_prefix(EDIT_PREFIX) {
        return open_issue_modal(ctx, data, component, key).await;
    }
    if custom_id != ASSIST_ID && custom_id != DRAFT_ID {
        return Ok(());
    }
    // Claude API と GitHub API を待つと初回応答の期限 (3 秒) を超えるので、先に保留する
    component.defer_ephemeral(ctx).await?;
    if !is_ticket(data, component.channel_id).await? {
        return edit(
            ctx,
            component,
            EditInteractionResponse::new()
                .content("記録対象のフォーラムの投稿 (チケット) の中で使ってください"),
        )
        .await;
    }
    let thread_id = snowflake(component.channel_id);
    let requested_by = snowflake(component.user.id);
    if custom_id == ASSIST_ID {
        run_assist(ctx, data, component, thread_id, requested_by).await
    } else {
        run_issue_draft(ctx, data, component, thread_id, requested_by).await
    }
}

/// DB からチケットを読み、Claude に送る形にする。削除済み・保存期間切れの本文は含めない。
/// スレッドが削除されていれば None
async fn load_ticket(data: &Data, thread_id: i64) -> Result<Option<TicketInput>, BotError> {
    let Some(thread) = store::thread(&data.pool, thread_id).await? else {
        return Ok(None);
    };
    if thread.deleted_at.is_some() {
        return Ok(None);
    }
    let rows = if thread.message_count > MAX_MESSAGES {
        // 最初の投稿 (問い合わせの本体) と、新しい側の発言 (省くのは古い側。claude::select_messages と同じ考え方)
        let mut rows = store::thread_messages(&data.pool, thread_id, 0, 1).await?;
        rows.extend(
            store::thread_messages(
                &data.pool,
                thread_id,
                thread.message_count - (MAX_MESSAGES - 1),
                MAX_MESSAGES - 1,
            )
            .await?,
        );
        rows
    } else {
        store::thread_messages(&data.pool, thread_id, 0, MAX_MESSAGES).await?
    };
    // 本文に書かれた Discord の ID (メンション・メッセージのリンク)・メールアドレス・招待リンクは、送る前に伏せる
    let messages = rows
        .into_iter()
        .filter_map(|row| {
            let content = row.content?;
            let attachments: Vec<String> = row
                .attachments
                .as_ref()
                .and_then(|a| a.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|a| a.get("filename").and_then(|f| f.as_str()))
                        .map(redact)
                        .collect()
                })
                .unwrap_or_default();
            if content.trim().is_empty() && attachments.is_empty() {
                return None;
            }
            let role = if row.author_is_staff {
                Role::Staff
            } else if Some(row.author_id) == thread.author_id {
                Role::Author
            } else {
                Role::User
            };
            Some(TicketMessage {
                role,
                created_at: row.created_at,
                content: redact(&content),
                attachments,
            })
        })
        .collect();
    Ok(Some(TicketInput {
        title: thread.title.as_deref().map(redact),
        tags: thread.tags,
        status_label: Status::from_db(&thread.status).label(),
        messages,
    }))
}

async fn record<T>(
    data: &Data,
    claude: &Claude,
    thread_id: i64,
    kind: &str,
    requested_by: i64,
    message_count: usize,
    outcome: &Outcome<T>,
) {
    let (label, usage) = match outcome {
        Outcome::Ok(_, usage) => ("ok", usage),
        Outcome::Refusal(usage) => ("refusal", usage),
        Outcome::Error(_, usage) => ("error", usage),
    };
    let request = db::AiRequest {
        thread_id,
        kind,
        requested_by,
        model: claude.model(),
        message_count: i32::try_from(message_count).unwrap_or(i32::MAX),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        outcome: label,
    };
    // 記録に失敗しても結果はスタッフに見せる (送信は済んでいるので)。失敗はエラーとして残す
    if let Err(error) = db::insert_ai_request(&data.pool, &request).await {
        tracing::error!(%error, thread_id, "failed to record a Claude API request");
    }
    tracing::info!(
        thread_id,
        kind,
        model = claude.model(),
        message_count,
        outcome = label,
        input_tokens = usage.input_tokens,
        output_tokens = usage.output_tokens,
        "sent a ticket to the Claude API"
    );
    if let Outcome::Error(error, _) = outcome {
        tracing::error!(%error, thread_id, kind, "Claude API request failed");
    }
}

async fn run_assist(
    ctx: &serenity::Context,
    data: &Data,
    component: &ComponentInteraction,
    thread_id: i64,
    requested_by: i64,
) -> Result<(), BotError> {
    let Some(settings) = data.settings.claude.clone() else {
        return edit(
            ctx,
            component,
            EditInteractionResponse::new().content("Claude API が未設定です"),
        )
        .await;
    };
    let ticket = match load_ticket(data, thread_id).await? {
        Some(ticket) if !ticket.messages.is_empty() => ticket,
        _ => {
            return edit(
                ctx,
                component,
                EditInteractionResponse::new()
                    .content("送れる本文がありません (削除済み・保存期間切れの発言は送りません)"),
            )
            .await;
        }
    };
    let claude = Claude::new(data.http.clone(), settings);
    let (outcome, count) = claude.assist(&ticket).await;
    record(
        data,
        &claude,
        thread_id,
        "assist",
        requested_by,
        count,
        &outcome,
    )
    .await;
    let response = match outcome {
        Outcome::Ok(assist, _) => {
            let omitted = ticket.messages.len() - count;
            let mut footer = format!("{} / 送った発言 {count} 件", claude.model());
            if omitted > 0 {
                footer.push_str(&format!(" (長いので古い発言 {omitted} 件を省略)"));
            }
            EditInteractionResponse::new()
                .content("Claude による案です。内容を確かめ、返信はあなたが自分で投稿してください (Bot は返信しません)")
                .embeds(vec![
                    CreateEmbed::new()
                        .title(format!("種類: {}", assist.category.label()))
                        .description(truncate(&assist.summary, MAX_SUMMARY_CHARS)),
                    CreateEmbed::new()
                        .title("返信案")
                        .description(truncate(&assist.reply_draft, MAX_REPLY_CHARS))
                        .footer(CreateEmbedFooter::new(footer)),
                ])
        }
        Outcome::Refusal(_) => EditInteractionResponse::new()
            .content("Claude がこのチケットへの応答を断りました。内容を確かめて、手で対応してください"),
        Outcome::Error(..) => EditInteractionResponse::new()
            .content("Claude API の呼び出しに失敗しました。時間をおいて再度お試しください (詳細は Bot のログ)"),
    };
    edit(ctx, component, response).await
}

async fn run_issue_draft(
    ctx: &serenity::Context,
    data: &Data,
    component: &ComponentInteraction,
    thread_id: i64,
    requested_by: i64,
) -> Result<(), BotError> {
    if data.settings.github.is_none() {
        return edit(
            ctx,
            component,
            EditInteractionResponse::new().content("GitHub が未設定です"),
        )
        .await;
    }
    if let Some(issue) = db::github_issue(&data.pool, thread_id).await? {
        return edit(
            ctx,
            component,
            EditInteractionResponse::new()
                .content(format!("このチケットは起票済みです (Issue #{issue})")),
        )
        .await;
    }
    let Some(ticket) = load_ticket(data, thread_id).await? else {
        return edit(
            ctx,
            component,
            EditInteractionResponse::new().content("このチケットは見つかりませんでした"),
        )
        .await;
    };
    let mut notes = Vec::new();
    let draft = match (&data.settings.claude, ticket.messages.is_empty()) {
        (Some(settings), false) => {
            let claude = Claude::new(data.http.clone(), settings.clone());
            let (outcome, count) = claude.issue_draft(&ticket).await;
            record(
                data,
                &claude,
                thread_id,
                "issue_draft",
                requested_by,
                count,
                &outcome,
            )
            .await;
            match outcome {
                Outcome::Ok(draft, _) => draft,
                Outcome::Refusal(_) | Outcome::Error(..) => {
                    notes.push(
                        "Claude で下書きを作れなかったので、最初の投稿からひな形を作りました",
                    );
                    template_draft(&ticket)
                }
            }
        }
        _ => template_draft(&ticket),
    };
    let pending = PendingIssue {
        thread_id,
        title: truncate_plain(
            &no_github_mentions(&redact(draft.title.trim())),
            MAX_TITLE_CHARS,
        ),
        body: format!(
            "{}{ISSUE_FOOTER}",
            truncate_plain(
                &no_github_mentions(&redact(draft.body.trim())),
                MAX_BODY_CHARS - ISSUE_FOOTER.chars().count()
            )
        ),
        created_at: Instant::now(),
    };
    let key = component.id.get();
    {
        let mut drafts = data.drafts.lock().expect("drafts lock poisoned");
        drafts.retain(|_, d| d.created_at.elapsed() < DRAFT_TTL);
        drafts.insert(key, pending.clone());
    }
    let mut content = vec![
        "Issue の下書きです。**公開リポジトリに載ります。** 名前・ID・メールアドレスなどの個人情報が残っていないか、".to_owned()
            + "「編集して起票」で開く画面で確かめ、必要なら直してから送信してください (30 分以内)。",
    ];
    content.extend(notes.iter().map(|n| format!("- {n}")));
    edit(
        ctx,
        component,
        EditInteractionResponse::new()
            .content(content.join("\n"))
            .embeds(vec![
                CreateEmbed::new()
                    .title(pending.title.clone())
                    .description(truncate(&pending.body, 4000)),
            ])
            .components(vec![CreateActionRow::Buttons(vec![
                CreateButton::new(format!("{EDIT_PREFIX}{key}"))
                    .label("編集して起票")
                    .style(ButtonStyle::Success),
            ])]),
    )
    .await
}

fn pending_issue(data: &Data, key: &str, thread_id: i64) -> Option<PendingIssue> {
    let key: u64 = key.parse().ok()?;
    let drafts = data.drafts.lock().expect("drafts lock poisoned");
    drafts
        .get(&key)
        .filter(|d| d.thread_id == thread_id && d.created_at.elapsed() < DRAFT_TTL)
        .cloned()
}

async fn open_issue_modal(
    ctx: &serenity::Context,
    data: &Data,
    component: &ComponentInteraction,
    key: &str,
) -> Result<(), BotError> {
    // モーダルは初回応答でしか開けないので、ここでは保留しない (DB も Discord API も呼ばない)
    let Some(pending) = pending_issue(data, key, snowflake(component.channel_id)) else {
        return reply_component(
            ctx,
            component,
            "下書きの期限が切れました。もう一度「Issue の下書き」から作り直してください",
        )
        .await;
    };
    let modal =
        CreateModal::new(format!("{MODAL_PREFIX}{key}"), "GitHub Issue を起票").components(vec![
            CreateActionRow::InputText(
                CreateInputText::new(InputTextStyle::Short, "タイトル", TITLE_INPUT)
                    .max_length(MAX_TITLE_CHARS as u16)
                    .value(pending.title),
            ),
            CreateActionRow::InputText(
                CreateInputText::new(
                    InputTextStyle::Paragraph,
                    "本文 (公開されます。個人情報を消してください)",
                    BODY_INPUT,
                )
                .max_length(MAX_BODY_CHARS as u16)
                .value(pending.body),
            ),
        ]);
    component
        .create_response(ctx, CreateInteractionResponse::Modal(modal))
        .await?;
    Ok(())
}

fn modal_value(modal: &ModalInteraction, id: &str) -> Option<String> {
    modal
        .data
        .components
        .iter()
        .flat_map(|row| row.components.iter())
        .find_map(|component| match component {
            ActionRowComponent::InputText(input) if input.custom_id == id => input.value.clone(),
            _ => None,
        })
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

async fn handle_modal(
    ctx: &serenity::Context,
    data: &Data,
    modal: &ModalInteraction,
) -> Result<(), BotError> {
    let Some(key) = modal.data.custom_id.strip_prefix(MODAL_PREFIX) else {
        return Ok(());
    };
    modal.defer_ephemeral(ctx).await?;
    let reply = |content: String| {
        EditInteractionResponse::new()
            .content(content)
            .allowed_mentions(CreateAllowedMentions::new())
    };
    let thread_id = snowflake(modal.channel_id);
    let Some(github) = data.settings.github.clone() else {
        modal
            .edit_response(ctx, reply("GitHub が未設定です".into()))
            .await?;
        return Ok(());
    };
    if pending_issue(data, key, thread_id).is_none() || !is_ticket(data, modal.channel_id).await? {
        modal
            .edit_response(
                ctx,
                reply(
                    "下書きの期限が切れました。もう一度「Issue の下書き」から作り直してください"
                        .into(),
                ),
            )
            .await?;
        return Ok(());
    }
    let (Some(title), Some(body)) = (
        modal_value(modal, TITLE_INPUT),
        modal_value(modal, BODY_INPUT),
    ) else {
        modal
            .edit_response(ctx, reply("タイトルと本文を入力してください".into()))
            .await?;
        return Ok(());
    };
    // 起票は直列にする (2 人が同時に送信して同じチケットから 2 件起票しないように)
    let _guard = data.issue_lock.lock().await;
    if let Some(issue) = db::github_issue(&data.pool, thread_id).await? {
        modal
            .edit_response(
                ctx,
                reply(format!("このチケットは起票済みです (Issue #{issue})")),
            )
            .await?;
        return Ok(());
    }
    let created = match github::create_issue(&data.http, &github, &title, &body).await {
        Ok(created) => created,
        Err(error) => {
            tracing::error!(%error, thread_id, "failed to create a GitHub issue");
            modal
                .edit_response(
                    ctx,
                    reply("GitHub への起票に失敗しました。トークンの権限と期限を確かめてください (詳細は Bot のログ)。通信が途中で切れた場合は起票されていることがあるので、リポジトリに同じ Issue が無いか確かめてから送り直してください".into()),
                )
                .await?;
            return Ok(());
        }
    };
    let requested_by = snowflake(modal.user.id);
    let saved = db::set_github_issue(
        &data.pool,
        thread_id,
        created.number,
        requested_by,
        Utc::now(),
    )
    .await;
    if let Err(error) = &saved {
        tracing::error!(%error, thread_id, issue = created.number, "failed to save the created issue number");
    }
    if let Ok(key) = key.parse::<u64>() {
        data.drafts
            .lock()
            .expect("drafts lock poisoned")
            .remove(&key);
    }
    tracing::info!(
        thread_id,
        issue = created.number,
        "created a GitHub issue from a ticket"
    );
    let mut message = format!(
        "Issue #{} を起票しました: {}",
        created.number, created.html_url
    );
    if saved.is_err() {
        message.push_str(
            "\n(起票した番号をチケットの記録に残せませんでした。Bot のログを確かめてください)",
        );
    }
    modal.edit_response(ctx, reply(message)).await?;
    Ok(())
}

/// Claude を使わないときの下書き (最初の投稿を引用したひな形)
fn template_draft(ticket: &TicketInput) -> claude::IssueDraft {
    let title = format!(
        "[bug] {}",
        ticket
            .title
            .as_deref()
            .unwrap_or("サポートに届いた不具合の報告")
    );
    let quoted = ticket
        .messages
        .first()
        .map(|m| {
            truncate_plain(&m.content, 2000)
                .lines()
                .map(|line| format!("> {line}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|| "(本文は削除済み、または保存期間を過ぎています)".into());
    let body = format!(
        "## 概要\n\n{quoted}\n\n## 再現手順\n\n1. \n\n## 期待する動作\n\n\n## 実際の動作\n\n\n## 環境\n\n- "
    );
    claude::IssueDraft { title, body }
}

/// 公開する下書きから、個人や場所を特定できる値を機械的に伏せる。
/// - メールアドレスらしき語 → `[メールアドレス]`、Discord の招待リンク → `[招待リンク]`
/// - 15〜21 桁の数字列 (Discord の ID。メンション `<@…>` やメッセージのリンクの中も含む) → `[ID]`
///
/// 名前などは機械的には見分けられないので、最後はスタッフがモーダルで確かめる
pub fn redact(text: &str) -> String {
    let words = redact_invites(text);
    let words = redact_emails(&words);
    let mut out = String::with_capacity(words.len());
    let mut digits = String::new();
    let flush = |digits: &mut String, out: &mut String| {
        if (15..=21).contains(&digits.len()) {
            out.push_str("[ID]");
        } else {
            out.push_str(digits);
        }
        digits.clear();
    };
    for c in words.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            flush(&mut digits, &mut out);
            out.push(c);
        }
    }
    flush(&mut digits, &mut out);
    out
}

/// GitHub の Issue に載せたときに、本文の `@name` で GitHub のユーザーやチームに通知が飛ばないよう、
/// `@` の直後に幅のない空白を挟む (見た目は変わらない)
pub fn no_github_mentions(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '@' && chars.peek().is_some_and(|n| n.is_ascii_alphanumeric()) {
            out.push('\u{200B}');
        }
    }
    out
}

/// Discord の招待リンクの部分だけを `[招待リンク]` に置き換える (前後に空白なしで続く本文は残す)
fn redact_invites(text: &str) -> String {
    const HOSTS: [&str; 3] = [
        "discord.gg/",
        "discord.com/invite/",
        "discordapp.com/invite/",
    ];
    const SCHEMES: [&str; 2] = ["https://", "http://"];
    // to_ascii_lowercase はバイト位置を変えないので、lower で見つけた位置をそのまま text に使える
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut rest = 0;
    let mut search = 0;
    while let Some((at, host)) = HOSTS
        .iter()
        .filter_map(|host| lower[search..].find(host).map(|i| (search + i, *host)))
        .min_by_key(|(i, _)| *i)
    {
        // 前に付いた https:// と www. も含める
        let mut start = at;
        if lower[..start].ends_with("www.") {
            start -= "www.".len();
        }
        if let Some(scheme) = SCHEMES.iter().find(|s| lower[..start].ends_with(*s)) {
            start -= scheme.len();
        }
        // 招待コードは英数字と `-` (全角の括弧や句読点で止まる)
        let code_start = at + host.len();
        let code_len = text[code_start..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
            .unwrap_or(text.len() - code_start);
        let end = code_start + code_len;
        if start < rest {
            start = rest;
        }
        out.push_str(&text[rest..start]);
        out.push_str("[招待リンク]");
        rest = end;
        search = end.max(code_start);
    }
    out.push_str(&text[rest..]);
    out
}

/// 本文中のメールアドレスを `[メールアドレス]` に置き換える。空白で区切られていなくても (`a@x.jp、b@y.jp` など)、
/// `@` の前後に続くメールアドレスに使える文字の範囲を 1 件ずつ見つけて置き換える
fn redact_emails(text: &str) -> String {
    let local_char = |c: char| c.is_ascii_alphanumeric() || "._%+-".contains(c);
    let domain_char = |c: char| c.is_ascii_alphanumeric() || ".-".contains(c);
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    // 直前に置き換えた範囲の終わり。ローカル部はここより前に戻って探さない (置き換え済みの文字を取り消さないため)
    let mut consumed = 0;
    while i < chars.len() {
        if chars[i] != '@' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // ローカル部: 直前まで書き出した文字のうち、使える文字が続く範囲 (先頭の記号は除く)
        let mut start = i;
        while start > consumed && local_char(chars[start - 1]) {
            start -= 1;
        }
        while start < i && !chars[start].is_ascii_alphanumeric() {
            start += 1;
        }
        // ドメイン部: 後ろに続く範囲 (末尾の `.` `-` は文の区切りとして除く)
        let mut end = i + 1;
        while end < chars.len() && domain_char(chars[end]) {
            end += 1;
        }
        while end > i + 1 && !chars[end - 1].is_ascii_alphanumeric() {
            end -= 1;
        }
        let domain: String = chars[i + 1..end].iter().collect();
        if start < i && domain.contains('.') && !domain.starts_with('.') && !domain.contains("..") {
            // 書き出し済みのローカル部を取り消して置き換える
            let local_len: usize = chars[start..i].iter().map(|c| c.len_utf8()).sum();
            out.truncate(out.len() - local_len);
            out.push_str("[メールアドレス]");
            i = end;
            consumed = end;
        } else {
            out.push('@');
            i += 1;
        }
    }
    out
}

/// 埋め込みの上限に収める (省いたことが分かるようにする)
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let kept: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}…")
}

fn truncate_plain(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_ids_emails_and_invites() {
        assert_eq!(
            redact("サーバー 1311111111111111111 で <@222222222222222222> が"),
            "サーバー [ID] で <@[ID]> が"
        );
        assert_eq!(
            redact(
                "https://discord.com/channels/1311111111111111111/1322222222222222222/1333333333333333333"
            ),
            "https://discord.com/channels/[ID]/[ID]/[ID]"
        );
        assert_eq!(
            redact("連絡先 (foo.bar+x@example.co.jp) まで"),
            "連絡先 ([メールアドレス]) まで"
        );
        assert_eq!(
            redact("招待 https://discord.gg/abcdef です"),
            "招待 [招待リンク] です"
        );
        assert_eq!(
            redact("改行\nfoo@example.com\n次"),
            "改行\n[メールアドレス]\n次"
        );
        // 空白で区切られていなくても 1 件ずつ伏せる
        assert_eq!(
            redact("foo@example.com、bar@example.org。連絡先:baz@test.jp."),
            "[メールアドレス]、[メールアドレス]。連絡先:[メールアドレス]."
        );
        // 置き換えた範囲を次の `@` のローカル部として数え直さない (以前は UTF-8 の境界でパニックした)
        assert_eq!(redact("a@b.com@c.com"), "[メールアドレス]@c.com");
        // 招待リンクの前後に空白なしで続く本文は残す
        assert_eq!(
            redact("招待リンク（https://discord.gg/abcdef）から参加すると表示されません。"),
            "招待リンク（[招待リンク]）から参加すると表示されません。"
        );
        assert_eq!(
            redact("HTTPS://www.Discord.com/invite/Ab-1、discord.gg/x"),
            "[招待リンク]、[招待リンク]"
        );
        assert_eq!(redact("あa@b.com@@x.jp"), "あ[メールアドレス]@@x.jp");
    }

    #[test]
    fn neutralizes_github_mentions() {
        assert_eq!(
            no_github_mentions("@octocat と @org/team"),
            "@\u{200B}octocat と @\u{200B}org/team"
        );
        assert_eq!(no_github_mentions("a @ b, @"), "a @ b, @");
    }

    #[test]
    fn keeps_ordinary_text() {
        for text in [
            "2026-11-28 09:00 に 3 件",
            "v3.31.1 で発生",
            "電話 09012345678",
            "@here と @everyone",
            "user@localhost",
        ] {
            assert_eq!(redact(text), text, "{text}");
        }
    }

    #[test]
    fn builds_a_template_from_the_first_post() {
        let ticket = TicketInput {
            title: Some("同期されない".into()),
            tags: vec![],
            status_label: "対応待ち",
            messages: vec![TicketMessage {
                role: Role::Author,
                created_at: Utc::now(),
                content: "予定が\n表示されない".into(),
                attachments: vec![],
            }],
        };
        let draft = template_draft(&ticket);
        assert_eq!(draft.title, "[bug] 同期されない");
        assert!(draft.body.contains("> 予定が\n> 表示されない"));
        assert!(draft.body.contains("## 再現手順"));
    }

    #[test]
    fn truncates_by_characters() {
        assert_eq!(truncate("あいうえお", 3), "あい…");
        assert_eq!(truncate("あいう", 3), "あいう");
        assert_eq!(truncate_plain("あいうえお", 2), "あい");
    }
}
