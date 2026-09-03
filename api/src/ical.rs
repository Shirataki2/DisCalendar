//! iCalendar (RFC 5545) の生成 (#95)。
//!
//! 外部カレンダー (Google / Apple / Outlook) が購読するフィードの本文を組み立てる。外部クレートは使わず、
//! 必要な要素 (VCALENDAR / VTIMEZONE / VEVENT、TEXT のエスケープ、75 オクテットでの行折り返し) だけを実装する。
//!
//! 予定は「JST の壁時計時刻」(`models` のモジュールコメント) なので、時刻指定の予定は `TZID=Asia/Tokyo` 付きで
//! そのまま出し、固定オフセットの `VTIMEZONE` を同梱する (TZID を参照するときは VTIMEZONE が必須。Outlook などは無いと読めない)。
//! 終日予定は `VALUE=DATE` で、`DTEND` は排他的なので終了日の翌日にする (DB の `end_at` は終了日の 0:00)

use chrono::{Duration, NaiveDate, NaiveDateTime};

use crate::models::{events::EventRow, guilds::Guild};

/// 時刻指定の予定に付けるタイムゾーン ID
pub const TZID: &str = "Asia/Tokyo";
/// 1 行の最大オクテット数 (CRLF を除く。RFC 5545 3.1)
const MAX_LINE_OCTETS: usize = 75;
/// `UID` のドメイン部。環境 (本番 / staging / ローカル) に関わらず同じ予定は同じ UID にする
const UID_DOMAIN: &str = "discalendar.app";

/// ギルドの予定を 1 つの VCALENDAR にする。`site_base_url` はカレンダー画面へのリンク (`URL`) の起点
pub fn render_feed(guild: &Guild, events: &[EventRow], site_base_url: &str) -> String {
    let mut out = String::new();
    push_line(&mut out, "BEGIN:VCALENDAR");
    push_line(&mut out, "VERSION:2.0");
    push_line(&mut out, "PRODID:-//DisCalendar//DisCalendar//JA");
    push_line(&mut out, "CALSCALE:GREGORIAN");
    push_line(&mut out, "METHOD:PUBLISH");
    // 購読したときのカレンダー名 (Apple / Google が読む拡張プロパティ)
    push_line(
        &mut out,
        &format!("X-WR-CALNAME:{}", escape_text(&guild.name)),
    );
    push_line(&mut out, &format!("X-WR-TIMEZONE:{TZID}"));
    // 更新間隔の希望 (RFC 7986 と旧来の拡張の両方)。クライアントが従うとは限らない
    push_line(&mut out, "REFRESH-INTERVAL;VALUE=DURATION:PT1H");
    push_line(&mut out, "X-PUBLISHED-TTL:PT1H");
    push_vtimezone(&mut out);
    let dashboard_url = format!("{site_base_url}/dashboard/{}", guild.guild_id);
    for event in events {
        push_vevent(&mut out, event, &dashboard_url);
    }
    push_line(&mut out, "END:VCALENDAR");
    out
}

/// Asia/Tokyo は夏時間が無いので STANDARD 1 つで表せる
fn push_vtimezone(out: &mut String) {
    push_line(out, "BEGIN:VTIMEZONE");
    push_line(out, &format!("TZID:{TZID}"));
    push_line(out, "BEGIN:STANDARD");
    push_line(out, "DTSTART:19700101T000000");
    push_line(out, "TZOFFSETFROM:+0900");
    push_line(out, "TZOFFSETTO:+0900");
    push_line(out, "TZNAME:JST");
    push_line(out, "END:STANDARD");
    push_line(out, "END:VTIMEZONE");
}

fn push_vevent(out: &mut String, event: &EventRow, dashboard_url: &str) {
    push_line(out, "BEGIN:VEVENT");
    push_line(out, &format!("UID:event-{}@{UID_DOMAIN}", event.id));
    // DTSTAMP は UTC でなければならない。予定には更新日時が無いので作成日時を使う
    // (内容が変わっても値は変わらないが、購読クライアントは本文の差分で更新を検出する)
    push_line(out, &format!("DTSTAMP:{}", format_utc(event.created_at)));
    push_line(out, &format!("SUMMARY:{}", escape_text(&event.name)));
    if let Some(description) = event.description.as_deref().filter(|d| !d.is_empty()) {
        push_line(out, &format!("DESCRIPTION:{}", escape_text(description)));
    }
    push_line(out, &format!("URL:{dashboard_url}"));
    if event.is_all_day {
        let start = event.start_at.date();
        // DTEND (VALUE=DATE) は排他的なので終了日の翌日。翌日を表せない終了日 (chrono の上限) は DTEND を省く
        // (省略時は DTSTART と同じ 1 日分と解釈される)
        let end = event.end_at.date().succ_opt();
        push_line(out, &format!("DTSTART;VALUE=DATE:{}", format_date(start)));
        if let Some(end) = end.filter(|end| *end > start) {
            push_line(out, &format!("DTEND;VALUE=DATE:{}", format_date(end)));
        }
    } else {
        push_line(
            out,
            &format!("DTSTART;TZID={TZID}:{}", format_local(event.start_at)),
        );
        // RFC 5545 は DTEND を DTSTART より後の時刻に限る。同時刻の予定は DTEND を省く
        // (省略時は開始と同じ時刻に終わる = 期間 0 として解釈される)
        if event.end_at > event.start_at {
            push_line(
                out,
                &format!("DTEND;TZID={TZID}:{}", format_local(event.end_at)),
            );
        }
    }
    push_line(out, "END:VEVENT");
}

/// JST の壁時計時刻をそのまま `YYYYMMDDTHHMMSS` にする (TZID 付きで使う)
fn format_local(dt: NaiveDateTime) -> String {
    dt.format("%Y%m%dT%H%M%S").to_string()
}

fn format_date(date: NaiveDate) -> String {
    date.format("%Y%m%d").to_string()
}

/// JST の壁時計時刻を UTC (`...Z`) にする
fn format_utc(jst: NaiveDateTime) -> String {
    (jst - Duration::hours(9))
        .format("%Y%m%dT%H%M%SZ")
        .to_string()
}

/// TEXT 値のエスケープ (RFC 5545 3.3.11)。改行は `\n` に、`\` `;` `,` はバックスラッシュで逃がす。
/// タブ以外の制御文字は TEXT に入れられないので落とす
fn escape_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\r' => {
                // CRLF は 1 つの改行として扱う
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push_str("\\n");
            }
            '\n' => out.push_str("\\n"),
            c if c.is_control() && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

/// 1 行を CRLF 終端で書く。75 オクテットを超える行は折り返す (継続行は先頭に空白 1 つ。RFC 5545 3.1)。
/// 折り返しはオクテット数で数えるが、UTF-8 の文字の途中では切らない
/// (切ると継続行を連結し直しても壊れたバイト列になる)
fn push_line(out: &mut String, line: &str) {
    let mut budget = MAX_LINE_OCTETS;
    let mut used = 0;
    for c in line.chars() {
        let len = c.len_utf8();
        if used + len > budget {
            out.push_str("\r\n ");
            used = 0;
            // 継続行は先頭の空白の分だけ短くなる
            budget = MAX_LINE_OCTETS - 1;
        }
        out.push(c);
        used += len;
    }
    out.push_str("\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guild() -> Guild {
        Guild {
            guild_id: "782502586817314816".into(),
            name: "ゲーム部".into(),
            avatar_url: None,
            locale: "ja".into(),
        }
    }

    fn event(is_all_day: bool, start: &str, end: &str) -> EventRow {
        EventRow {
            id: 42,
            guild_id: "782502586817314816".into(),
            name: "定例ミーティング".into(),
            description: Some("議題;\n1, 2\\3".into()),
            notifications: serde_json::json!([]),
            color: "#2196F3".into(),
            is_all_day,
            start_at: start.parse().unwrap(),
            end_at: end.parse().unwrap(),
            created_at: "2026-08-01T00:00:00".parse().unwrap(),
            discord_scheduled_event_id: None,
        }
    }

    /// 折り返しを戻して 1 行ずつにする (検証用)
    fn unfold(ics: &str) -> Vec<String> {
        ics.replace("\r\n ", "")
            .split("\r\n")
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn renders_calendar_header_and_timezone() {
        let ics = render_feed(&guild(), &[], "https://discalendar.app");
        let lines = unfold(&ics);
        assert_eq!(lines.first().map(String::as_str), Some("BEGIN:VCALENDAR"));
        assert_eq!(lines.last().map(String::as_str), Some("END:VCALENDAR"));
        assert!(lines.contains(&"VERSION:2.0".to_owned()));
        assert!(lines.contains(&"X-WR-CALNAME:ゲーム部".to_owned()));
        assert!(lines.contains(&"TZID:Asia/Tokyo".to_owned()));
        assert!(lines.contains(&"TZOFFSETTO:+0900".to_owned()));
        // 行末は CRLF
        assert!(ics.ends_with("END:VCALENDAR\r\n"));
        assert!(!ics.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn renders_timed_event_with_tzid() {
        let ics = render_feed(
            &guild(),
            &[event(false, "2026-08-22T10:00:00", "2026-08-22T11:30:00")],
            "https://discalendar.app",
        );
        let lines = unfold(&ics);
        assert!(lines.contains(&"UID:event-42@discalendar.app".to_owned()));
        assert!(lines.contains(&"DTSTART;TZID=Asia/Tokyo:20260822T100000".to_owned()));
        assert!(lines.contains(&"DTEND;TZID=Asia/Tokyo:20260822T113000".to_owned()));
        assert!(lines.contains(&"SUMMARY:定例ミーティング".to_owned()));
        // TEXT のエスケープ: ; , \ と改行
        assert!(lines.contains(&"DESCRIPTION:議題\\;\\n1\\, 2\\\\3".to_owned()));
        assert!(
            lines.contains(&"URL:https://discalendar.app/dashboard/782502586817314816".to_owned())
        );
    }

    #[test]
    fn dtstamp_is_created_at_in_utc() {
        let ics = render_feed(
            &guild(),
            &[event(false, "2026-08-22T10:00:00", "2026-08-22T11:00:00")],
            "https://discalendar.app",
        );
        // 2026-08-01 00:00 JST = 2026-07-31 15:00 UTC
        assert!(unfold(&ics).contains(&"DTSTAMP:20260731T150000Z".to_owned()));
    }

    #[test]
    fn omits_dtend_when_event_has_no_duration() {
        let ics = render_feed(
            &guild(),
            &[event(false, "2026-08-22T10:00:00", "2026-08-22T10:00:00")],
            "https://discalendar.app",
        );
        assert!(!ics.contains("DTEND"));
    }

    #[test]
    fn renders_all_day_event_with_exclusive_end_date() {
        // DB では終日予定の end_at は終了日の 0:00 (8/22〜8/23 の 2 日間)
        let ics = render_feed(
            &guild(),
            &[event(true, "2026-08-22T00:00:00", "2026-08-23T00:00:00")],
            "https://discalendar.app",
        );
        let lines = unfold(&ics);
        assert!(lines.contains(&"DTSTART;VALUE=DATE:20260822".to_owned()));
        assert!(lines.contains(&"DTEND;VALUE=DATE:20260824".to_owned()));
        assert!(!ics.contains("TZID=Asia/Tokyo:"));

        // 1 日だけの終日予定も翌日を DTEND にする
        let ics = render_feed(
            &guild(),
            &[event(true, "2026-08-22T00:00:00", "2026-08-22T00:00:00")],
            "https://discalendar.app",
        );
        assert!(unfold(&ics).contains(&"DTEND;VALUE=DATE:20260823".to_owned()));
    }

    #[test]
    fn omits_description_when_empty() {
        let mut e = event(false, "2026-08-22T10:00:00", "2026-08-22T11:00:00");
        e.description = None;
        let ics = render_feed(&guild(), &[e], "https://discalendar.app");
        assert!(!ics.contains("DESCRIPTION"));
        let mut e = event(false, "2026-08-22T10:00:00", "2026-08-22T11:00:00");
        e.description = Some(String::new());
        let ics = render_feed(&guild(), &[e], "https://discalendar.app");
        assert!(!ics.contains("DESCRIPTION"));
    }

    #[test]
    fn folds_long_lines_at_utf8_boundaries() {
        let mut out = String::new();
        // 8 + 30 × 3 = 98 オクテット
        let line = format!("SUMMARY:{}", "あ".repeat(30));
        push_line(&mut out, &line);
        let physical: Vec<&str> = out.split("\r\n").filter(|l| !l.is_empty()).collect();
        assert!(physical.len() > 1);
        for (i, l) in physical.iter().enumerate() {
            assert!(
                l.len() <= MAX_LINE_OCTETS,
                "line {i} has {} octets",
                l.len()
            );
            if i > 0 {
                assert!(l.starts_with(' '));
            }
        }
        // 継続行を連結し直すと元に戻る
        assert_eq!(out.replace("\r\n ", "").trim_end_matches("\r\n"), line);

        // ちょうど 75 オクテットは折り返さない
        let mut out = String::new();
        push_line(&mut out, &"a".repeat(MAX_LINE_OCTETS));
        assert_eq!(out, format!("{}\r\n", "a".repeat(MAX_LINE_OCTETS)));
    }

    #[test]
    fn escape_text_handles_control_characters() {
        assert_eq!(escape_text("a\r\nb\rc\nd"), "a\\nb\\nc\\nd");
        assert_eq!(escape_text("tab\tok\u{7}bell"), "tab\tokbell");
        assert_eq!(escape_text("plain"), "plain");
    }
}
