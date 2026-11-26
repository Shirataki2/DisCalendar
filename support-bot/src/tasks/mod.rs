//! 定期タスク。どれも `tokio::time::interval` で回し、失敗しても警告を残して次の周期で再試行する
//! (`bot/src/tasks/history_cleanup.rs` と同じ形)。

pub mod daily_summary;
pub mod reminders;
pub mod retention;
pub mod unreplied;

use std::time::Duration;

use poise::serenity_prelude::{
    self as serenity, CreateAllowedMentions, CreateEmbed, CreateMessage,
};

use crate::data::Data;

pub fn spawn_all(ctx: serenity::Context, data: Data) {
    tokio::spawn(unreplied::run_loop(ctx.clone(), data.clone()));
    tokio::spawn(reminders::run_loop(ctx.clone(), data.clone()));
    tokio::spawn(daily_summary::run_loop(ctx, data.clone()));
    tokio::spawn(retention::run_loop(data));
}

pub(crate) fn interval(period: Duration) -> tokio::time::Interval {
    let mut interval = tokio::time::interval(period);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval
}

/// スタッフ用チャンネルに投稿する。メンションは `mention_users` に渡した人だけに飛ばす
/// (チケットのスレッド名やメモに @everyone などが含まれていても通知を飛ばさないため)
pub(crate) async fn post_to_staff(
    ctx: &serenity::Context,
    data: &Data,
    content: Option<String>,
    embed: CreateEmbed,
    mention_users: Vec<serenity::UserId>,
) -> anyhow::Result<()> {
    let mut message = CreateMessage::new()
        .embed(embed)
        .allowed_mentions(CreateAllowedMentions::new().users(mention_users));
    if let Some(content) = content {
        message = message.content(content);
    }
    data.settings
        .staff_channel_id
        .send_message(ctx, message)
        .await?;
    Ok(())
}

/// スレッドへのリンク (`<#id>`) とスレッド名。名前は保存期間で消えていることがある
pub(crate) fn thread_line(thread_id: i64, title: Option<&str>) -> String {
    match title {
        Some(title) => format!("<#{thread_id}> {}", sanitize(title, 60)),
        None => format!("<#{thread_id}>"),
    }
}

/// 埋め込みに載せる利用者の文字列。長すぎるものは切り、Markdown のリンクや強調・メンションの記法が
/// 効かないよう記号をエスケープする
pub(crate) fn sanitize(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, c) in text.chars().enumerate() {
        if i == max_chars {
            out.push('…');
            break;
        }
        if c == '\n' || c == '\r' {
            out.push(' ');
            continue;
        }
        if matches!(
            c,
            '\\' | '*' | '_' | '~' | '`' | '|' | '[' | ']' | '<' | '>' | '#' | '@'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// 埋め込みの説明文の上限 (4096 文字) に収まるよう、行を詰めて残りの件数を添える
pub(crate) fn join_lines(lines: &[String], limit: usize) -> String {
    join_lines_counted(lines, limit).0
}

/// `join_lines` と同じく詰め、本文に載せられた行数も返す (先頭から何行が載ったか)
pub(crate) fn join_lines_counted(lines: &[String], limit: usize) -> (String, usize) {
    let tail = |rest: usize| format!("\n…ほか {rest} 件");
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        // この行の後ろに残る行があれば、「ほか N 件」を書ける分を空けておく
        let rest_after = lines.len() - i - 1;
        let reserve = if rest_after == 0 {
            0
        } else {
            tail(rest_after).chars().count()
        };
        let separator = usize::from(!out.is_empty());
        if out.chars().count() + separator + line.chars().count() + reserve > limit {
            // 1 つ前の行で tail(len - i) 分を空けてあるので必ず収まる
            out.push_str(&tail(lines.len() - i));
            return (out.trim_start().to_owned(), i);
        }
        if separator == 1 {
            out.push('\n');
        }
        out.push_str(line);
    }
    (out, lines.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_user_text() {
        assert_eq!(sanitize("**太字** [x](y)", 100), r"\*\*太字\*\* \[x\](y)");
        assert_eq!(sanitize("@everyone", 100), r"\@everyone");
        assert_eq!(sanitize("改行\nあり", 100), "改行 あり");
        assert_eq!(sanitize("あいうえお", 3), "あいう…");
    }

    #[test]
    fn joins_lines_within_limit() {
        let lines: Vec<String> = (0..5).map(|i| format!("line{i}")).collect();
        assert_eq!(
            join_lines(&lines, 1000),
            "line0\nline1\nline2\nline3\nline4"
        );
        let joined = join_lines(&lines, 25);
        assert!(joined.chars().count() <= 25, "{joined}");
        assert!(joined.ends_with("件"), "{joined}");
        let (text, shown) = join_lines_counted(&lines, 25);
        assert_eq!(
            text.lines().filter(|l| l.starts_with("line")).count(),
            shown
        );
        assert!(shown < lines.len());
        assert_eq!(join_lines_counted(&lines, 1000).1, lines.len());
    }
}
