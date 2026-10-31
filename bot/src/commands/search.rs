//! `/search`: サーバー内の予定をキーワードで検索し、選んだ予定の詳細を表示する (#206)。
//!
//! 結果は実行した本人にだけ表示する。5 件ずつ「前へ」「次へ」でページ送りし、セレクトメニューで選ぶと
//! 詳細 (名称・説明・開始終了・場所・通知) とカレンダーを開くリンクに切り替わる。
//! 共有リンクは発行済みのときだけ案内し、Bot からは発行しない (通知と同じ)

use std::time::Duration;

use poise::serenity_prelude::{self as serenity, CreateActionRow, CreateButton, CreateEmbed};

use crate::{
    data::Context,
    error::BotError,
    i18n::Locale,
    models::{
        event_share_links,
        events::{self, Event, SearchScope},
        now_jst,
    },
};

/// 1 ページに表示する候補の数 (セレクトメニューの選択肢の上限 25 より十分小さく、埋め込みが長くなりすぎない数)
const PER_PAGE: usize = 5;
/// キーワードの最大文字数
const KEYWORD_MAX_CHARS: usize = 100;
/// 操作がないときにボタンを外すまでの時間 (interaction トークンの有効期限 15 分より短くする)
const IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// 予定をキーワードで検索します
#[poise::command(slash_command, guild_only, ephemeral)]
pub async fn search(
    ctx: Context<'_>,
    #[description = "検索するキーワード (タイトル・説明の部分一致)"]
    #[max_length = 100]
    keyword: String,
    #[description = "検索する予定の範囲 (省略時は未来)"] range: Option<SearchRange>,
) -> Result<(), BotError> {
    let locale = crate::i18n::user_locale(ctx);
    let Some(guild_id) = ctx.guild_id() else {
        return Err(crate::user_error!(
            "このコマンドはサーバー内でのみ実行できます",
            "This command can only be used in a server."
        ));
    };
    // 入力の誤りは defer 前に本人にだけ返す
    let keyword = normalize_keyword(&keyword)?;
    let range = range.unwrap_or_default();
    let guild_id = guild_id.to_string();
    let pool = &ctx.data().pool;

    ctx.defer_ephemeral().await?;

    let now = now_jst();
    crate::recurring_events::ensure_range(
        pool,
        &guild_id,
        now - chrono::Duration::days(366),
        now + chrono::Duration::days(730),
    )
    .await?;
    // 上限 + 1 件読み、上限を超えたかどうかを判定する
    let mut results = events::search(
        pool,
        &guild_id,
        &keyword,
        range.scope(),
        now,
        events::SEARCH_LIMIT + 1,
    )
    .await?;
    let truncated = results.len() > events::SEARCH_LIMIT as usize;
    results.truncate(events::SEARCH_LIMIT as usize);
    if results.is_empty() {
        ctx.say(crate::tr!(
            locale,
            "「{keyword}」に一致する予定はありません (範囲: {})。キーワードや範囲を変えてお試しください",
            "No events match \"{keyword}\" (range: {}). Try another keyword or range.",
            crate::messages::message(locale, range.label())
        ))
        .await?;
        return Ok(());
    }

    let mut view = SearchView {
        keyword,
        range,
        results,
        truncated,
        page: 0,
        locale,
    };
    let ids = ComponentIds::new(ctx.id());
    // 時間切れでボタンを外すときに再指定する、表示中の埋め込み。
    // poise の `CreateReply` での編集は空の `embeds` も送るため、指定しないと内容が消える
    let mut shown = view.list_embed(None);
    let reply = ctx
        .send(
            poise::CreateReply::default()
                .embed(shown.clone())
                .components(view.list_components(&ids)),
        )
        .await?;
    let message_id = reply.message().await?.id;
    // 最後に応答した操作。時間切れの編集はこのトークンで行う。最初のコマンドのトークンは 15 分で切れるため、
    // 操作を続けた後の時間切れでは使えない (操作のトークンは、その操作から 15 分は有効)
    let mut last_press: Option<serenity::ComponentInteraction> = None;

    loop {
        let press = serenity::ComponentInteractionCollector::new(ctx.serenity_context())
            .author_id(ctx.author().id)
            .channel_id(ctx.channel_id())
            .message_id(message_id)
            .filter({
                let ids = ids.clone();
                move |press| ids.contains(&press.data.custom_id)
            })
            .timeout(IDLE_TIMEOUT)
            .await;
        let Some(press) = press else {
            // 無操作タイムアウト: 内容は残してボタンとメニューだけ外す
            let removed = match &last_press {
                Some(press) => press
                    .edit_response(
                        ctx.http(),
                        serenity::EditInteractionResponse::new()
                            .embed(shown)
                            .components(Vec::new()),
                    )
                    .await
                    .map(|_| ()),
                None => {
                    reply
                        .edit(
                            ctx,
                            poise::CreateReply::default()
                                .embed(shown)
                                .components(Vec::new()),
                        )
                        .await
                }
            };
            if let Err(error) = removed {
                tracing::warn!(%error, "failed to remove search components");
            }
            return Ok(());
        };

        // 候補がすべて削除されたら、更新後に操作を待たずに終える
        let mut finished = false;
        let (embed, components) = if press.data.custom_id == ids.prev {
            view.page = view.page.saturating_sub(1);
            (view.list_embed(None), view.list_components(&ids))
        } else if press.data.custom_id == ids.next {
            view.page = (view.page + 1).min(view.total_pages() - 1);
            (view.list_embed(None), view.list_components(&ids))
        } else if press.data.custom_id == ids.back {
            (view.list_embed(None), view.list_components(&ids))
        } else {
            let selected = match &press.data.kind {
                serenity::ComponentInteractionDataKind::StringSelect { values } => {
                    values.first().and_then(|value| value.parse::<i32>().ok())
                }
                _ => None,
            };
            // 一覧を出した後に削除・変更されている場合があるので、選んだ時点の内容を読み直す。
            // guild_id でも絞るので、他のサーバーの予定は取得できない
            let found = match selected {
                Some(id) => events::find(pool, &guild_id, id).await?,
                None => None,
            };
            match found {
                Some(event) => {
                    let token = event_share_links::get_token(pool, &guild_id, event.id).await?;
                    let links =
                        DetailLinks::new(&ctx.data().site_base_url, &event, token.as_deref());
                    (
                        detail_embed(&event, &links, locale),
                        detail_components(&ids, &links, locale),
                    )
                }
                None => {
                    if let Some(id) = selected {
                        view.remove(id);
                    }
                    let notice = locale.text(
                        "選択した予定は削除されたため表示できません",
                        "The selected event has been deleted.",
                    );
                    finished = view.results.is_empty();
                    let components = if finished {
                        Vec::new()
                    } else {
                        view.list_components(&ids)
                    };
                    (view.list_embed(Some(notice)), components)
                }
            }
        };
        press
            .create_response(
                ctx.http(),
                serenity::CreateInteractionResponse::UpdateMessage(
                    serenity::CreateInteractionResponseMessage::new()
                        .embed(embed.clone())
                        .components(components),
                ),
            )
            .await?;
        shown = embed;
        last_press = Some(press);
        if finished {
            return Ok(());
        }
    }
}

/// 前後の空白を除き、空なら案内する
fn normalize_keyword(keyword: &str) -> Result<String, BotError> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Err(crate::user_error!(
            "検索するキーワードを入力してください",
            "Please enter a keyword to search for."
        ));
    }
    if keyword.chars().count() > KEYWORD_MAX_CHARS {
        return Err(crate::user_error!(
            "キーワードは {} 文字以内で入力してください",
            "Keywords must be {} characters or fewer.",
            KEYWORD_MAX_CHARS
        ));
    }
    Ok(keyword.to_owned())
}

/// `/search` の範囲。`/list` の過去・未来・全てと同じ判定
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, poise::ChoiceParameter)]
pub enum SearchRange {
    #[default]
    #[name = "未来"]
    Future,
    #[name = "過去"]
    Past,
    #[name = "全て"]
    All,
}

impl SearchRange {
    fn scope(self) -> SearchScope {
        match self {
            Self::Future => SearchScope::Future,
            Self::Past => SearchScope::Past,
            Self::All => SearchScope::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Future => "未来",
            Self::Past => "過去",
            Self::All => "全て",
        }
    }
}

/// 検索結果の一覧の状態
struct SearchView {
    keyword: String,
    range: SearchRange,
    results: Vec<Event>,
    truncated: bool,
    /// 0 始まり
    page: usize,
    locale: Locale,
}

impl SearchView {
    fn total_pages(&self) -> usize {
        self.results.len().div_ceil(PER_PAGE).max(1)
    }

    /// 現在のページの (通し番号 (1 始まり), 予定)
    fn page_items(&self) -> impl Iterator<Item = (usize, &Event)> {
        self.results
            .iter()
            .enumerate()
            .skip(self.page * PER_PAGE)
            .take(PER_PAGE)
            .map(|(i, event)| (i + 1, event))
    }

    /// 削除された予定を候補から外し、ページが範囲外になったら最後のページに戻す
    fn remove(&mut self, id: i32) {
        self.results.retain(|event| event.id != id);
        self.page = self.page.min(self.total_pages() - 1);
    }

    fn list_embed(&self, notice: Option<&str>) -> CreateEmbed {
        let locale = self.locale;
        let mut description = crate::tr!(
            locale,
            "範囲: {}　該当: {} 件{}\n候補を選ぶと詳細を表示します",
            "Range: {}  Matches: {}{}\nSelect an event to see its details.",
            crate::messages::message(locale, self.range.label()),
            self.results.len(),
            if self.truncated {
                crate::tr!(
                    locale,
                    " (先頭 {} 件のみ表示。キーワードで絞り込んでください)",
                    " (showing the first {} only; please narrow your keyword)",
                    events::SEARCH_LIMIT
                )
            } else {
                String::new()
            }
        );
        if let Some(notice) = notice {
            description = format!("⚠️ {notice}\n{description}");
        }
        let mut embed = CreateEmbed::new()
            .title(crate::tr!(
                locale,
                "「{}」の検索結果",
                "Search results for \"{}\"",
                self.keyword
            ))
            .description(description)
            .colour(0x0000ff)
            .footer(serenity::CreateEmbedFooter::new(format!(
                "Page {}/{}",
                self.page + 1,
                self.total_pages()
            )));
        for (number, event) in self.page_items() {
            embed = embed.field(
                format!("{number}. {}", event.name),
                super::list::describe_localized(event, locale),
                false,
            );
        }
        embed
    }

    fn list_components(&self, ids: &ComponentIds) -> Vec<CreateActionRow> {
        let locale = self.locale;
        let options = self
            .page_items()
            .map(|(number, event)| {
                serenity::CreateSelectMenuOption::new(
                    truncate(&format!("{number}. {}", event.name), 100),
                    event.id.to_string(),
                )
                .description(truncate(&plain_date_range(event, locale), 100))
            })
            .collect();
        vec![
            CreateActionRow::SelectMenu(
                serenity::CreateSelectMenu::new(
                    &ids.select,
                    serenity::CreateSelectMenuKind::String { options },
                )
                .placeholder(locale.text("詳細を表示する予定を選択", "Select an event")),
            ),
            CreateActionRow::Buttons(vec![
                CreateButton::new(&ids.prev)
                    .style(serenity::ButtonStyle::Primary)
                    .label(crate::messages::message(locale, "前へ"))
                    .disabled(self.page == 0),
                CreateButton::new(&ids.next)
                    .style(serenity::ButtonStyle::Primary)
                    .label(crate::messages::message(locale, "次へ"))
                    .disabled(self.page + 1 >= self.total_pages()),
            ]),
        ]
    }
}

/// セレクトメニューの説明に使う日時。メニューでは Discord のタイムスタンプ表記が使えないので JST で書く
fn plain_date_range(event: &Event, locale: Locale) -> String {
    if event.is_all_day {
        let start = locale.date(event.start_at);
        let all_day = locale.text("終日", "all day");
        if event.start_at.date() == event.end_at.date() {
            format!("{start} ({all_day})")
        } else {
            format!("{start} - {} ({all_day})", locale.date(event.end_at))
        }
    } else if event.start_at.date() == event.end_at.date() {
        format!(
            "{} - {} JST",
            event
                .start_at
                .format(locale.text("%Y/%m/%d %H:%M", "%b %d, %Y %H:%M")),
            event.end_at.format("%H:%M")
        )
    } else {
        format!(
            "{} - {}",
            locale.datetime(event.start_at),
            locale.datetime(event.end_at)
        )
    }
}

/// 文字数 (Unicode スカラー値) で切り詰める。Discord の上限を超えると送信が 400 になる
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut truncated: String = text.chars().take(max - 1).collect();
    truncated.push('…');
    truncated
}

/// 詳細から辿る URL。共有リンクは発行済みの場合だけ案内する (公開範囲は既存の共有リンクの仕様のまま)
struct DetailLinks {
    calendar: String,
    share: Option<String>,
}

impl DetailLinks {
    fn new(site_base_url: &str, event: &Event, token: Option<&str>) -> Self {
        let base = site_base_url.trim_end_matches('/');
        Self {
            // web のカレンダーは `date` と `event` で該当日を開いて予定を選んだ状態にする (ログインとサーバーの閲覧権限が要る)
            calendar: format!(
                "{base}/dashboard/{}?date={}&event={}",
                event.guild_id,
                event.start_at.format("%Y-%m-%d"),
                event.id
            ),
            share: token.map(|token| format!("{base}/share/{token}")),
        }
    }
}

fn detail_embed(event: &Event, links: &DetailLinks, locale: Locale) -> CreateEmbed {
    let color = u32::from_str_radix(event.color.trim_start_matches('#'), 16).unwrap_or(0x0000ff);
    let description = event
        .description
        .as_deref()
        .filter(|description| !description.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| locale.text("説明はありません", "No description").to_owned());
    let mut embed = CreateEmbed::new()
        .title(&event.name)
        .url(&links.calendar)
        .description(description)
        .colour(color)
        .field(
            crate::messages::message(locale, "開始"),
            crate::datetime::format_datetime_localized(event.start_at, event.is_all_day, locale),
            false,
        )
        .field(
            crate::messages::message(locale, "終了"),
            crate::datetime::format_datetime_localized(event.end_at, event.is_all_day, locale),
            false,
        );
    if let Some(location) = &event.location {
        embed = embed.field(crate::messages::message(locale, "場所"), location, false);
    }
    embed.field(
        crate::messages::message(locale, "通知"),
        super::list::notifications_text(event, locale),
        false,
    )
}

fn detail_components(
    ids: &ComponentIds,
    links: &DetailLinks,
    locale: Locale,
) -> Vec<CreateActionRow> {
    let mut buttons = vec![
        CreateButton::new(&ids.back)
            .style(serenity::ButtonStyle::Secondary)
            .label(locale.text("検索結果に戻る", "Back to results")),
        CreateButton::new_link(&links.calendar)
            .label(locale.text("カレンダーで開く", "Open calendar")),
    ];
    if let Some(url) = &links.share {
        buttons.push(CreateButton::new_link(url).label(locale.text("詳細を見る", "View details")));
    }
    vec![CreateActionRow::Buttons(buttons)]
}

/// 他のコマンド・他の実行のボタンと区別できるよう、この呼び出し固有の ID を付ける
#[derive(Debug, Clone)]
struct ComponentIds {
    select: String,
    prev: String,
    next: String,
    back: String,
}

impl ComponentIds {
    fn new(invocation_id: u64) -> Self {
        Self {
            select: format!("{invocation_id}:search:select"),
            prev: format!("{invocation_id}:search:prev"),
            next: format!("{invocation_id}:search:next"),
            back: format!("{invocation_id}:search:back"),
        }
    }

    fn contains(&self, id: &str) -> bool {
        [&self.select, &self.prev, &self.next, &self.back]
            .iter()
            .any(|known| *known == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    fn event(id: i32, name: &str, start: &str, end: &str, all_day: bool) -> Event {
        Event {
            id,
            guild_id: "1".to_owned(),
            name: name.to_owned(),
            description: None,
            location: None,
            notifications: serde_json::json!([]),
            notification_mentions: serde_json::json!([]),
            color: "#2196F3".to_owned(),
            is_all_day: all_day,
            start_at: start.parse::<NaiveDateTime>().unwrap(),
            end_at: end.parse::<NaiveDateTime>().unwrap(),
            created_at: "2026-08-01T00:00:00".parse().unwrap(),
            created_by: None,
            updated_by: None,
            updated_at: None,
        }
    }

    fn view(count: i32) -> SearchView {
        SearchView {
            keyword: "定例".to_owned(),
            range: SearchRange::Future,
            results: (1..=count)
                .map(|i| {
                    event(
                        i,
                        "定例",
                        "2026-08-23T10:00:00",
                        "2026-08-23T11:00:00",
                        false,
                    )
                })
                .collect(),
            truncated: false,
            page: 0,
            locale: Locale::Ja,
        }
    }

    #[test]
    fn keyword_is_trimmed_and_must_not_be_blank() {
        assert_eq!(normalize_keyword("  定例 ").unwrap(), "定例");
        for blank in ["", "   ", "\u{3000}"] {
            let Err(BotError::User(message)) = normalize_keyword(blank) else {
                panic!("blank keyword must be rejected: {blank:?}");
            };
            assert_eq!(message.ja, "検索するキーワードを入力してください");
        }
        assert!(normalize_keyword(&"あ".repeat(KEYWORD_MAX_CHARS)).is_ok());
        assert!(normalize_keyword(&"あ".repeat(KEYWORD_MAX_CHARS + 1)).is_err());
    }

    #[test]
    fn ranges_match_list_scopes_and_default_to_future() {
        assert_eq!(SearchRange::default(), SearchRange::Future);
        assert_eq!(SearchRange::Future.scope(), SearchScope::Future);
        assert_eq!(SearchRange::Past.scope(), SearchScope::Past);
        assert_eq!(SearchRange::All.scope(), SearchScope::All);
    }

    #[test]
    fn pages_hold_five_items_numbered_across_pages() {
        let mut v = view(11);
        assert_eq!(v.total_pages(), 3);
        assert_eq!(
            v.page_items().map(|(n, _)| n).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
        v.page = 2;
        assert_eq!(
            v.page_items().map(|(n, e)| (n, e.id)).collect::<Vec<_>>(),
            [(11, 11)]
        );
        // 最後のページの唯一の候補が削除されたら前のページに戻る
        v.remove(11);
        assert_eq!(v.page, 1);
        assert_eq!(v.results.len(), 10);
        v.remove(999);
        assert_eq!(v.results.len(), 10);
    }

    #[test]
    fn same_named_events_are_distinguished_by_plain_jst_dates() {
        let timed = event(
            1,
            "定例",
            "2026-08-23T10:00:00",
            "2026-08-23T11:30:00",
            false,
        );
        assert_eq!(
            plain_date_range(&timed, Locale::Ja),
            "2026/08/23 10:00 - 11:30 JST"
        );
        assert_eq!(
            plain_date_range(&timed, Locale::En),
            "Aug 23, 2026 10:00 - 11:30 JST"
        );
        let overnight = event(
            2,
            "定例",
            "2026-08-23T23:00:00",
            "2026-08-24T01:00:00",
            false,
        );
        assert_eq!(
            plain_date_range(&overnight, Locale::Ja),
            "2026/08/23 23:00 JST - 2026/08/24 01:00 JST"
        );
        let all_day = event(
            3,
            "合宿",
            "2026-08-23T00:00:00",
            "2026-08-25T00:00:00",
            true,
        );
        assert_eq!(
            plain_date_range(&all_day, Locale::Ja),
            "2026/08/23 - 2026/08/25 (終日)"
        );
        let one_day = event(
            4,
            "休み",
            "2026-08-23T00:00:00",
            "2026-08-23T00:00:00",
            true,
        );
        assert_eq!(plain_date_range(&one_day, Locale::Ja), "2026/08/23 (終日)");
    }

    #[test]
    fn truncate_counts_characters() {
        assert_eq!(truncate("あいう", 3), "あいう");
        assert_eq!(truncate("あいうえ", 3), "あい…");
        assert_eq!(truncate(&"x".repeat(150), 100).chars().count(), 100);
    }

    #[test]
    fn detail_links_open_the_event_and_only_show_issued_share_links() {
        let e = event(
            42,
            "定例",
            "2026-08-23T10:00:00",
            "2026-08-23T11:00:00",
            false,
        );
        let links = DetailLinks::new("https://discalendar.app/", &e, None);
        assert_eq!(
            links.calendar,
            "https://discalendar.app/dashboard/1?date=2026-08-23&event=42"
        );
        assert_eq!(links.share, None);
        let links = DetailLinks::new("https://discalendar.app", &e, Some("abc"));
        assert_eq!(
            links.share.as_deref(),
            Some("https://discalendar.app/share/abc")
        );
    }

    #[test]
    fn component_ids_are_scoped_to_the_invocation() {
        let ids = ComponentIds::new(7);
        for suffix in ["select", "prev", "next", "back"] {
            assert!(ids.contains(&format!("7:search:{suffix}")));
        }
        assert!(!ids.contains("8:search:select"));
        assert!(!ids.contains("7:prev"));
    }
}
