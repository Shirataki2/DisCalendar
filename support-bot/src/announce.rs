//! 本番に出た版の更新内容を、サポートサーバーの「更新通知」チャンネルへ告知する (#430)。
//!
//! 本番デプロイ (Actions の "Deploy production") でコンテナが新しい版に入れ替わり、起動したときに自分の版
//! (`CARGO_PKG_VERSION`) を見て告知する。タグを打った時点ではまだ本番に出ていないので、デプロイ後の起動を合図にする。
//!
//! - 告知済みの版はサポート用 DB (`support_release_announcements`) に持つ。そこにある版より新しい正式版のときだけ告知する
//!   (再起動・デプロイの再実行で同じ版を二度出さず、前の版へのロールバックで古い版を告知し直さない)
//! - プレリリース (`3.x.y-rc.1`) は告知しない
//! - 本文は更新履歴 (`web/src/content/changelog.mdx`) のその版の節。ビルド時に埋め込む。節が無い版
//!   (利用者に見える変更が無いリリース) は投稿せず、告知済みとして記録だけ残す
//! - 告知先がアナウンスチャンネルなら公開 (crosspost) して、フォローしているサーバーにも届ける

use std::time::Duration;

use poise::serenity_prelude::{
    self as serenity, ChannelId, ChannelType, CreateAllowedMentions, CreateEmbed, CreateMessage,
};

use crate::{data::Data, db, record::snowflake};

/// 自分の版。web / api / bot / support-bot で共通 (リリースのたびに揃えて上げる)
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// 更新履歴の原本。support-bot/Dockerfile でもこのファイルをコピーしている
const CHANGELOG: &str = include_str!("../../web/src/content/changelog.mdx");
const SITE_URL: &str = "https://discalendar.app";
const CHANGELOG_URL: &str = "https://discalendar.app/changelog";
/// 埋め込みの説明文の上限 (Discord は 4096 文字)
const DESCRIPTION_LIMIT: usize = 4096;
/// 投稿の失敗 (Discord の一時的な不調など) をその場で再試行する回数と間隔
const ATTEMPTS: u32 = 3;
const RETRY_AFTER: Duration = Duration::from_secs(60);

/// 起動時に一度だけ呼ぶ。告知先が未設定なら何もしない
pub async fn run(ctx: serenity::Context, data: Data) {
    let Some(channel_id) = data.settings.release_channel_id else {
        tracing::debug!("SUPPORT_RELEASE_CHANNEL_ID is not set; skipping the release announcement");
        return;
    };
    for attempt in 1..=ATTEMPTS {
        match announce(&ctx, &data, channel_id).await {
            Ok(()) => return,
            Err(error) if attempt < ATTEMPTS => {
                tracing::warn!(%error, attempt, version = CURRENT_VERSION, "failed to announce the release; retrying");
                tokio::time::sleep(RETRY_AFTER).await;
            }
            Err(error) => {
                // 記録は取り消してあるので、次の起動 (docker compose restart support-bot) で告知し直す
                tracing::error!(%error, version = CURRENT_VERSION, "failed to announce the release");
            }
        }
    }
}

async fn announce(
    ctx: &serenity::Context,
    data: &Data,
    channel_id: ChannelId,
) -> anyhow::Result<()> {
    let announced = db::announced_versions(&data.pool).await?;
    if !should_announce(CURRENT_VERSION, &announced) {
        tracing::debug!(
            version = CURRENT_VERSION,
            "the release is already announced (or is older / a pre-release)"
        );
        return Ok(());
    }
    // 投稿の前に記録を取る (取れなければ他が告知済み)。途中で落ちて投稿したか分からなくなったときは、
    // 二重に告知するより告知しない方を選ぶ
    if !db::claim_release_announcement(&data.pool, CURRENT_VERSION).await? {
        return Ok(());
    }
    let Some(section) = changelog_section(CHANGELOG, CURRENT_VERSION) else {
        tracing::info!(
            version = CURRENT_VERSION,
            "no user-facing changes in the changelog; not announcing this release"
        );
        return Ok(());
    };
    let message = match post(ctx, channel_id, &section).await {
        Ok(message) => message,
        Err(error) => {
            db::release_announcement_claim(&data.pool, CURRENT_VERSION).await?;
            return Err(error);
        }
    };
    tracing::info!(
        version = CURRENT_VERSION,
        message_id = message.id.get(),
        "announced the release"
    );
    // 投稿できたら、記録の保存より先に公開する。投稿前に取った記録で告知済みの扱いになっているので、
    // 保存の失敗で戻ると再試行も再起動も告知を飛ばし、公開されないまま終わるため
    crosspost_if_announcement(ctx, channel_id, &message).await;
    // メッセージ ID は後から辿るための付記。保存に失敗しても告知は済んでいるので、エラーを残すだけにする
    // (Err を返すと投稿をやり直そうとするが、記録があるので何もしない)
    if let Err(error) = db::set_release_announcement_message(
        &data.pool,
        CURRENT_VERSION,
        snowflake(channel_id),
        snowflake(message.id),
    )
    .await
    {
        tracing::error!(%error, version = CURRENT_VERSION, "failed to save the announced message");
    }
    Ok(())
}

async fn post(
    ctx: &serenity::Context,
    channel_id: ChannelId,
    section: &Section,
) -> anyhow::Result<serenity::Message> {
    let message = CreateMessage::new()
        .embed(announcement_embed(section))
        // 更新履歴に @everyone のような文字列が含まれても通知を飛ばさない
        .allowed_mentions(CreateAllowedMentions::new());
    Ok(channel_id.send_message(ctx, message).await?)
}

/// アナウンスチャンネルなら公開する。失敗しても投稿自体は済んでいるので、警告だけ残す
/// (手で「公開」を押せば届く)
async fn crosspost_if_announcement(
    ctx: &serenity::Context,
    channel_id: ChannelId,
    message: &serenity::Message,
) {
    let is_news = match channel_id.to_channel(ctx).await {
        Ok(serenity::Channel::Guild(channel)) => channel.kind == ChannelType::News,
        Ok(_) => false,
        Err(error) => {
            tracing::warn!(%error, "failed to fetch the release channel; not crossposting");
            return;
        }
    };
    if !is_news {
        return;
    }
    match message.crosspost(ctx).await {
        Ok(_) => tracing::info!(
            message_id = message.id.get(),
            "crossposted the release announcement"
        ),
        Err(error) => tracing::warn!(%error, "failed to crosspost the release announcement"),
    }
}

/// `3.31.0` の形の正式版を (major, minor, patch) にする。プレリリース (`-rc.1`) やビルドメタデータ付きは None
pub fn parse_release(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim().trim_start_matches('v').split('.');
    let mut next = || parts.next()?.parse::<u64>().ok();
    let parsed = (next()?, next()?, next()?);
    parts.next().is_none().then_some(parsed)
}

/// 告知するか。正式版で、告知済みのどの版よりも新しいときだけ true
/// (同じ版は二度告知せず、前の版へ戻したときも告知しない)
pub fn should_announce(current: &str, announced: &[String]) -> bool {
    let Some(current) = parse_release(current) else {
        return false;
    };
    announced
        .iter()
        .filter_map(|v| parse_release(v))
        .all(|v| v < current)
}

/// 更新履歴のある版の節
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub version: String,
    /// 見出しの括弧内 (`2026年11月27日`)
    pub date: Option<String>,
    /// Discord 向けに整形した本文
    pub body: String,
}

/// 更新履歴から `## vX.Y.Z (日付)` の節を取り出して Discord 向けに整形する。節が無い・中身が空なら None
pub fn changelog_section(mdx: &str, version: &str) -> Option<Section> {
    let mdx = strip_mdx_comments(mdx);
    let heading = format!("## v{version}");
    let mut lines = mdx.lines();
    let date = lines.by_ref().find_map(|line| {
        let rest = line.strip_prefix(&heading)?;
        // 3.1.0 の見出しで 3.1.0x などを拾わないよう、版の直後は空白か行末に限る
        if !(rest.is_empty() || rest.starts_with(' ')) {
            return None;
        }
        Some(
            rest.trim()
                .strip_prefix('(')
                .and_then(|r| r.strip_suffix(')'))
                .map(str::to_owned),
        )
    })?;
    let body: Vec<&str> = lines.take_while(|line| !line.starts_with("## ")).collect();
    let body = to_discord(&body.join("\n"));
    (!body.is_empty()).then(|| Section {
        version: version.to_owned(),
        date,
        body,
    })
}

/// `{/* ... */}` の MDX コメントを落とす (複数行にまたがるものも)
fn strip_mdx_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{/*") {
        out.push_str(&rest[..start]);
        match rest[start..].find("*/}") {
            Some(end) => rest = &rest[start + end + 3..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// 更新履歴の Markdown (MDX) を Discord の埋め込みで読める形にする。
/// - `###` (変更のまとまり) は太字、`####` (Web / Bot の区分) は下線にする (埋め込みでは見出し記法が効かないため)
/// - JSX のコンポーネント・import / export の行は落とす
/// - サイト内のリンク (`/docs/...`) は URL に直す
/// - 空行の連続は 1 行にまとめる
pub fn to_discord(markdown: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        let is_jsx = trimmed.starts_with('<')
            && trimmed[1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/');
        if is_jsx || trimmed.starts_with("import ") || trimmed.starts_with("export ") {
            continue;
        }
        let line = if let Some(title) = line.strip_prefix("#### ") {
            format!("__{}__", title.trim())
        } else if let Some(title) = line.strip_prefix("### ") {
            format!("**{}**", title.trim())
        } else {
            line.trim_end().to_owned()
        };
        let line = line.replace("](/", &format!("]({SITE_URL}/"));
        if line.is_empty() && out.last().is_none_or(|last| last.is_empty()) {
            continue;
        }
        out.push(line);
    }
    while out.last().is_some_and(|last| last.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// 説明文を `limit` 文字に収める。長いときは行の切れ目で切り、続きは更新履歴へ案内する。
/// 末尾には常に更新履歴へのリンクを付ける
pub fn fit_description(body: &str, limit: usize) -> String {
    let link = format!("\n\n📝 [更新履歴をすべて見る]({CHANGELOG_URL})");
    let more = format!("\n\n…続きは[更新履歴]({CHANGELOG_URL})をご覧ください。");
    let len = |s: &str| s.chars().count();
    if len(body) + len(&link) <= limit {
        return format!("{body}{link}");
    }
    let budget = limit.saturating_sub(len(&more));
    let mut out = String::new();
    for line in body.lines() {
        let separator = usize::from(!out.is_empty());
        if len(&out) + separator + len(line) > budget {
            break;
        }
        if separator == 1 {
            out.push('\n');
        }
        out.push_str(line);
    }
    if out.is_empty() {
        // 1 行目だけで上限を超えるときは文字の途中で切る
        out = body.chars().take(budget).collect();
    }
    format!("{}{more}", out.trim_end())
}

pub fn announcement_embed(section: &Section) -> CreateEmbed {
    let title = match &section.date {
        Some(date) => format!("📢 DisCalendar v{} ({date}) の更新内容", section.version),
        None => format!("📢 DisCalendar v{} の更新内容", section.version),
    };
    CreateEmbed::new()
        .title(title)
        .url(CHANGELOG_URL)
        .description(fit_description(&section.body, DESCRIPTION_LIMIT))
        .colour(0x5865f2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn versions(list: &[&str]) -> Vec<String> {
        list.iter().map(|v| (*v).to_owned()).collect()
    }

    #[test]
    fn parses_only_releases() {
        assert_eq!(parse_release("3.31.0"), Some((3, 31, 0)));
        assert_eq!(parse_release("v3.31.2"), Some((3, 31, 2)));
        assert_eq!(parse_release("3.31.0-rc.1"), None);
        assert_eq!(parse_release("3.31"), None);
        assert_eq!(parse_release("3.31.0.1"), None);
    }

    #[test]
    fn announces_only_newer_releases() {
        // まだ何も告知していなければ告知する
        assert!(should_announce("3.31.0", &[]));
        // 新しい版
        assert!(should_announce("3.31.0", &versions(&["3.30.0", "3.29.1"])));
        // 版の比較は数値で行う (文字列だと 3.9.0 > 3.10.0 になる)
        assert!(should_announce("3.10.0", &versions(&["3.9.0"])));
        // 再起動・デプロイの再実行
        assert!(!should_announce("3.31.0", &versions(&["3.31.0"])));
        // ロールバック
        assert!(!should_announce("3.30.0", &versions(&["3.31.0"])));
        // プレリリース
        assert!(!should_announce("3.32.0-rc.1", &versions(&["3.31.0"])));
        assert!(!should_announce("3.32.0-rc.1", &[]));
    }

    const SAMPLE: &str = r#"{/*
  更新履歴 (利用者向け)。
  ## v9.9.9 (コメント内の見出しは拾わない)
*/}

DisCalendar の新機能や改善のお知らせ。

## v3.31.0 (2026年11月28日)

### 更新内容を告知するようにしました

#### Web (ブラウザ)

- [使い方](/docs/notify) を更新しました。 {/* 行内のコメント */}
<Callout type="info">
- 箇条書き


#### Bot (Discord)

- `/help` を直しました。


## v3.30.0 (2026年11月27日)

### 前の版

- 前の版の変更
"#;

    #[test]
    fn extracts_and_formats_the_section() {
        let section = changelog_section(SAMPLE, "3.31.0").unwrap();
        assert_eq!(section.date.as_deref(), Some("2026年11月28日"));
        assert_eq!(
            section.body,
            "**更新内容を告知するようにしました**\n\n__Web (ブラウザ)__\n\n\
             - [使い方](https://discalendar.app/docs/notify) を更新しました。\n- 箇条書き\n\n\
             __Bot (Discord)__\n\n- `/help` を直しました。"
        );
        let older = changelog_section(SAMPLE, "3.30.0").unwrap();
        assert_eq!(older.body, "**前の版**\n\n- 前の版の変更");
    }

    #[test]
    fn missing_or_empty_sections_are_none() {
        // 利用者に見える変更が無い版は見出しが無い
        assert_eq!(changelog_section(SAMPLE, "3.31.1"), None);
        // コメントの中の見出しは拾わない
        assert_eq!(changelog_section(SAMPLE, "9.9.9"), None);
        // 3.31.0 で 3.31.01 のような見出しを拾わない / 中身が空なら None
        assert_eq!(
            changelog_section("## v3.31.01 (x)\n- a\n## v3.31.0 (y)\n\n", "3.31.0"),
            None
        );
    }

    #[test]
    fn real_changelog_has_released_sections() {
        // 埋め込んだ更新履歴が読めること (v3.30.0 は利用者に見える変更があった版)
        let section = changelog_section(CHANGELOG, "3.30.0").unwrap();
        assert!(section.body.starts_with("**"), "{}", section.body);
        assert!(!section.body.contains("{/*"));
        assert!(!section.body.contains("](/"));
    }

    #[test]
    fn fits_within_the_limit() {
        let short = fit_description("- a", 4096);
        assert!(short.starts_with("- a\n\n📝 [更新履歴をすべて見る]"));

        let long: String = (0..500).map(|i| format!("- 変更 {i}\n")).collect();
        let fitted = fit_description(&long, 4096);
        assert!(fitted.chars().count() <= 4096, "{}", fitted.chars().count());
        assert!(fitted.ends_with("をご覧ください。"));
        // 行の途中で切らない
        assert!(fitted.contains("- 変更 0\n- 変更 1\n"));
        let kept = fitted.lines().filter(|l| l.starts_with("- 変更")).count();
        assert!(kept > 100 && kept < 500, "{kept}");

        // 1 行だけで上限を超えるときも収める
        let one_line = "あ".repeat(5000);
        assert!(fit_description(&one_line, 4096).chars().count() <= 4096);
    }
}
