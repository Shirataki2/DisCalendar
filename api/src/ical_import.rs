//! ICS ファイルの解析と、DisCalendar の予定入力への変換。
//!
//! ファイル取り込みと外部カレンダー購読で変換規則を共有できるよう、HTTP や DB には依存させない。

use std::collections::{BTreeMap, HashSet};

use chrono::{Datelike, Duration, FixedOffset, NaiveDate, NaiveDateTime};
use mailrs_ical::{CalDateTime, EventStatus, ParsedInvite, RawComponent, VTimezone};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    models::{
        events::{DESCRIPTION_MAX_CHARS, EventInput, NAME_MAX_CHARS},
        now_jst,
    },
    recurrence::{self, Ending, Rule},
    recurring::ChangeScope,
};

pub const ICS_MAX_BYTES: usize = 1024 * 1024;
pub const IMPORT_MAX_EVENTS: usize = 200;
pub const IMPORT_MAX_OCCURRENCES: usize = 10_000;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ImportEventInput {
    #[schema(example = "大会日程")]
    pub name: String,
    pub description: Option<String>,
    #[schema(example = "#2196F3")]
    pub color: String,
    pub is_all_day: bool,
    #[schema(example = "2026-10-01T10:00:00")]
    pub start_at: NaiveDateTime,
    #[schema(example = "2026-10-01T11:00:00")]
    pub end_at: NaiveDateTime,
    #[serde(default, rename = "recurrence_rule")]
    #[schema(value_type = Option<Object>)]
    pub recurrence: Option<Rule>,
}

impl ImportEventInput {
    pub fn to_event_input(&self) -> EventInput {
        EventInput {
            recurrence: self.recurrence.clone(),
            scope: ChangeScope::This,
            expected_series_version: None,
            name: self.name.clone(),
            description: self.description.clone(),
            location: None,
            notifications: Vec::new(),
            notification_mentions: Some(Vec::new()),
            color: self.color.clone(),
            is_all_day: self.is_all_day,
            start_at: self.start_at,
            end_at: self.end_at,
            discord_scheduled_event: Some(false),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ParsedImportEvent {
    /// ファイル内の VEVENT の位置 (1 始まり)
    pub source_index: usize,
    pub event: ImportEventInput,
    /// `name` / `description` のうち上限に合わせて切り詰めた項目
    pub truncated_fields: Vec<String>,
    /// 初回保存時に実体化される予定数の見込み
    pub estimated_occurrences: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SkippedImport {
    pub reason: String,
    pub count: usize,
}

#[derive(Debug)]
pub struct ParsedCalendar {
    pub events: Vec<ParsedImportEvent>,
    pub total_count: usize,
    pub skipped: Vec<SkippedImport>,
}

fn skip(counts: &mut BTreeMap<&'static str, usize>, reason: &'static str) {
    *counts.entry(reason).or_default() += 1;
}

/// 構文が壊れたファイルだけをエラーにし、個々の扱えない VEVENT は理由別にスキップする。
pub fn parse(contents: &str) -> Result<ParsedCalendar, String> {
    if contents.len() > ICS_MAX_BYTES {
        return Err("ICS ファイルは 1 MiB 以下にしてください".into());
    }
    let root = mailrs_ical::parse::parse_calendar(contents)
        .map_err(|error| format!("ICS ファイルを解析できません: {error:?}"))?;
    let RawComponent {
        name,
        properties,
        children,
    } = root;
    let (event_components, timezone_components): (Vec<_>, Vec<_>) = children
        .into_iter()
        .filter(|child| {
            child.name.eq_ignore_ascii_case("VEVENT")
                || child.name.eq_ignore_ascii_case("VTIMEZONE")
        })
        .partition(|child| child.name.eq_ignore_ascii_case("VEVENT"));
    let total_count = event_components.len();

    let mut lifted = Vec::with_capacity(total_count);
    let mut exceptional_uids = HashSet::new();
    let mut counts = BTreeMap::new();
    let mut calendar = RawComponent {
        name,
        properties,
        children: Vec::with_capacity(1),
    };
    let mut first_valid_event = None;
    for (index, event) in event_components.into_iter().enumerate() {
        calendar.children.clear();
        calendar.children.push(event);
        match mailrs_ical::semantics::lift(&calendar) {
            Ok(invite) => {
                first_valid_event.get_or_insert_with(|| calendar.children[0].clone());
                if invite.recurrence_id.is_some()
                    || !invite.exdate.is_empty()
                    || !invite.rdate.is_empty()
                {
                    exceptional_uids.insert(invite.uid.clone());
                }
                lifted.push((index + 1, invite));
            }
            Err(_) => skip(&mut counts, "invalid_event"),
        }
    }
    let vtimezones = match first_valid_event {
        Some(event) if !timezone_components.is_empty() => {
            calendar.children = std::iter::once(event).chain(timezone_components).collect();
            match mailrs_ical::semantics::lift(&calendar) {
                Ok(invite) => invite.vtimezones,
                Err(_) => {
                    for _ in 0..lifted.len() {
                        skip(&mut counts, "invalid_event");
                    }
                    lifted.clear();
                    Vec::new()
                }
            }
        }
        _ => Vec::new(),
    };

    let mut events = Vec::new();
    for (source_index, invite) in lifted {
        let reason = if exceptional_uids.contains(&invite.uid) {
            Some("recurrence_exceptions")
        } else if invite.status == Some(EventStatus::Cancelled) {
            Some("cancelled")
        } else {
            None
        };
        if let Some(reason) = reason {
            skip(&mut counts, reason);
            continue;
        }
        match convert(source_index, invite, &vtimezones) {
            Ok(event) => events.push(event),
            Err(reason) => skip(&mut counts, reason),
        }
    }

    Ok(ParsedCalendar {
        events,
        total_count,
        skipped: counts
            .into_iter()
            .map(|(reason, count)| SkippedImport {
                reason: reason.to_owned(),
                count,
            })
            .collect(),
    })
}

fn convert(
    source_index: usize,
    invite: ParsedInvite,
    vtimezones: &[VTimezone],
) -> Result<ParsedImportEvent, &'static str> {
    let (start_at, end_at, is_all_day) = convert_dates(&invite, vtimezones)?;
    let recurrence = invite
        .rrule
        .as_deref()
        .map(|raw| parse_rule(raw, start_at))
        .transpose()
        .map_err(|_| "unsupported_recurrence")?;

    let mut truncated_fields = Vec::new();
    let name = truncate(
        &invite.summary,
        NAME_MAX_CHARS,
        &mut truncated_fields,
        "name",
    );
    if name.trim().is_empty() {
        return Err("missing_title");
    }
    let description = invite.description.as_deref().map(|value| {
        truncate(
            value,
            DESCRIPTION_MAX_CHARS,
            &mut truncated_fields,
            "description",
        )
    });
    let event = ImportEventInput {
        name,
        description,
        color: "#2196F3".into(),
        is_all_day,
        start_at,
        end_at,
        recurrence,
    };
    let estimated_occurrences =
        estimate_occurrences(&event).map_err(|_| "unsupported_recurrence")?;
    Ok(ParsedImportEvent {
        source_index,
        event,
        truncated_fields,
        estimated_occurrences,
    })
}

fn truncate(value: &str, max: usize, fields: &mut Vec<String>, field: &str) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }
    fields.push(field.to_owned());
    value.chars().take(max).collect()
}

fn convert_dates(
    invite: &ParsedInvite,
    vtimezones: &[VTimezone],
) -> Result<(NaiveDateTime, NaiveDateTime, bool), &'static str> {
    match invite.dtstart {
        CalDateTime::Date(start) => {
            let end = match invite.dtend.as_ref() {
                Some(CalDateTime::Date(exclusive)) => exclusive.pred_opt().ok_or("invalid_date")?,
                Some(_) => return Err("invalid_date"),
                None => invite
                    .duration
                    .map(|duration| {
                        start
                            .checked_add_signed(duration - Duration::days(1))
                            .ok_or("invalid_date")
                    })
                    .transpose()?
                    .unwrap_or(start),
            };
            if end < start {
                return Err("invalid_date");
            }
            Ok((midnight(start)?, midnight(end)?, true))
        }
        _ => {
            let start = to_jst(&invite.dtstart, vtimezones)?;
            let end = match invite.dtend.as_ref() {
                Some(CalDateTime::Date(_)) => return Err("invalid_date"),
                Some(value) => to_jst(value, vtimezones)?,
                None => invite
                    .duration
                    .and_then(|duration| start.checked_add_signed(duration))
                    .unwrap_or(start),
            };
            if end < start {
                return Err("invalid_date");
            }
            Ok((start, end, false))
        }
    }
}

fn midnight(date: NaiveDate) -> Result<NaiveDateTime, &'static str> {
    date.and_hms_opt(0, 0, 0).ok_or("invalid_date")
}

fn to_jst(value: &CalDateTime, vtimezones: &[VTimezone]) -> Result<NaiveDateTime, &'static str> {
    match value {
        CalDateTime::Floating(value) => Ok(*value),
        CalDateTime::Date(date) => midnight(*date),
        _ => {
            let utc = mailrs_ical::vtimezone::caldatetime_to_utc(value, vtimezones)
                .ok_or("unknown_timezone")?;
            Ok(utc
                .with_timezone(&FixedOffset::east_opt(9 * 3600).expect("JST offset is valid"))
                .naive_local())
        }
    }
}

fn parse_rule(raw: &str, start: NaiveDateTime) -> Result<Rule, String> {
    let mut parts = BTreeMap::new();
    for part in raw.split(';') {
        let (key, value) = part
            .split_once('=')
            .ok_or_else(|| "繰り返し条件を解釈できません".to_owned())?;
        let key = key.to_ascii_uppercase();
        if parts.insert(key, value.to_ascii_uppercase()).is_some() {
            return Err("繰り返し条件を解釈できません".into());
        }
    }
    let allowed = [
        "FREQ",
        "INTERVAL",
        "BYDAY",
        "BYMONTHDAY",
        "COUNT",
        "UNTIL",
        "WKST",
    ];
    if parts.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("対応していない繰り返し条件です".into());
    }
    let interval = parts
        .get("INTERVAL")
        .map(|value| value.parse::<u8>())
        .transpose()
        .map_err(|_| "繰り返し間隔が不正です")?
        .unwrap_or(1);
    let mut rule = match parts.get("FREQ").map(String::as_str) {
        Some("DAILY")
            if interval == 1
                && !parts.contains_key("BYDAY")
                && !parts.contains_key("BYMONTHDAY") =>
        {
            Rule::Daily { end: Ending::Never }
        }
        Some("WEEKLY") if matches!(interval, 1 | 2) && !parts.contains_key("BYMONTHDAY") => {
            if interval == 2 && parts.get("WKST").is_some_and(|value| value != "MO") {
                return Err("隔週の週開始曜日は月曜だけに対応しています".into());
            }
            let weekdays = match parts.get("BYDAY") {
                Some(value) => value
                    .split(',')
                    .map(parse_weekday)
                    .collect::<Result<Vec<_>, _>>()?,
                None => vec![start.weekday().num_days_from_monday() as u8],
            };
            if interval == 1 {
                Rule::Weekly {
                    weekdays,
                    end: Ending::Never,
                }
            } else {
                Rule::Biweekly {
                    weekdays,
                    end: Ending::Never,
                }
            }
        }
        Some("MONTHLY") if interval == 1 => match (parts.get("BYMONTHDAY"), parts.get("BYDAY")) {
            (None, None) => Rule::MonthlyDate {
                day: start.day() as u8,
                end: Ending::Never,
            },
            (Some(day), None) if !day.contains(',') => Rule::MonthlyDate {
                day: day.parse().map_err(|_| "毎月の日付が不正です")?,
                end: Ending::Never,
            },
            (None, Some(day)) if !day.contains(',') && day.len() >= 3 => {
                let (nth, weekday) = day.split_at(day.len() - 2);
                Rule::MonthlyWeekday {
                    nth: nth.parse().map_err(|_| "毎月の第n曜日が不正です")?,
                    weekday: parse_weekday(weekday)?,
                    end: Ending::Never,
                }
            }
            _ => return Err("対応していない毎月の繰り返し条件です".into()),
        },
        _ => return Err("対応していない繰り返し条件です".into()),
    };
    apply_ending(&mut rule, parts.get("COUNT"), parts.get("UNTIL"), start)?;
    rule.rrule(start)?;
    Ok(rule)
}

fn apply_ending(
    rule: &mut Rule,
    count: Option<&String>,
    until: Option<&String>,
    start: NaiveDateTime,
) -> Result<(), String> {
    match (count, until) {
        (Some(_), Some(_)) => Err("COUNT と UNTIL は同時に指定できません".into()),
        (Some(count), None) => {
            *rule.ending_mut().expect("recurring rule has an ending") = Ending::Count {
                count: count.parse().map_err(|_| "繰り返し回数が不正です")?,
            };
            Ok(())
        }
        (None, Some(until)) => {
            let boundary = if until.ends_with('Z') {
                NaiveDateTime::parse_from_str(until, "%Y%m%dT%H%M%SZ")
                    .map_err(|_| "繰り返し終了日が不正です")?
                    .and_utc()
                    .with_timezone(&FixedOffset::east_opt(9 * 3600).expect("JST offset is valid"))
                    .naive_local()
            } else if until.len() == 8 {
                NaiveDate::parse_from_str(until, "%Y%m%d")
                    .map_err(|_| "繰り返し終了日が不正です")?
                    .and_hms_opt(23, 59, 59)
                    .ok_or("繰り返し終了日が不正です")?
            } else {
                NaiveDateTime::parse_from_str(until, "%Y%m%dT%H%M%S")
                    .map_err(|_| "繰り返し終了日が不正です")?
            };
            let from = boundary
                .checked_sub_signed(Duration::days(370))
                .unwrap_or(NaiveDateTime::MIN)
                .max(start);
            let to = boundary
                .checked_add_signed(Duration::seconds(1))
                .ok_or("繰り返し終了日が不正です")?;
            let last = recurrence::between(&rule.rrule(start)?, start, from, to)?
                .last()
                .copied()
                .ok_or("繰り返し終了日が開始日時より前です")?;
            *rule.ending_mut().expect("recurring rule has an ending") =
                Ending::Until { date: last.date() };
            Ok(())
        }
        (None, None) => Ok(()),
    }
}

fn parse_weekday(value: &str) -> Result<u8, String> {
    match value {
        "MO" => Ok(0),
        "TU" => Ok(1),
        "WE" => Ok(2),
        "TH" => Ok(3),
        "FR" => Ok(4),
        "SA" => Ok(5),
        "SU" => Ok(6),
        _ => Err("曜日が不正です".into()),
    }
}

pub fn estimate_occurrences(event: &ImportEventInput) -> Result<usize, String> {
    let Some(rule) = event.recurrence.as_ref().filter(|rule| rule.enabled()) else {
        return Ok(1);
    };
    let text = rule.rrule(event.start_at)?;
    let now = now_jst();
    let overlap = event.end_at - event.start_at
        + if event.is_all_day {
            Duration::days(1)
        } else {
            Duration::zero()
        };
    let from = now.checked_sub_signed(overlap).ok_or("期間が不正です")?;
    let to = now
        .checked_add_signed(Duration::days(recurrence::LOOKAHEAD_DAYS + 1))
        .ok_or("期間が不正です")?;
    let starts = recurrence::between(&text, event.start_at, from.max(event.start_at), to)?;
    Ok(starts.len() + usize::from(!starts.contains(&event.start_at)))
}

pub fn overlaps(
    event: &ImportEventInput,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<bool, String> {
    let range_start = start.and_hms_opt(0, 0, 0).expect("midnight is valid");
    let range_end = end
        .succ_opt()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .unwrap_or(NaiveDateTime::MAX);
    let event_end = if event.is_all_day {
        event
            .end_at
            .checked_add_signed(Duration::days(1))
            .unwrap_or(NaiveDateTime::MAX)
    } else {
        event.end_at
    };
    if event.start_at < range_end && event_end >= range_start {
        return Ok(true);
    }
    let Some(rule) = event.recurrence.as_ref().filter(|rule| rule.enabled()) else {
        return Ok(false);
    };
    let duration = event_end - event.start_at;
    let from = range_start
        .checked_sub_signed(duration)
        .unwrap_or(NaiveDateTime::MIN)
        .max(event.start_at);
    // 対応する繰り返しは最長でも月単位なので、次の1回の有無は1年見れば判定できる。
    let to = range_end.min(
        range_start
            .checked_add_signed(Duration::days(370))
            .unwrap_or(NaiveDateTime::MAX),
    );
    if to <= from {
        return Ok(false);
    }
    Ok(!recurrence::between(&rule.rrule(event.start_at)?, event.start_at, from, to)?.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str =
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nMETHOD:PUBLISH\r\nPRODID:-//test//EN\r\n";
    const FOOTER: &str = "END:VCALENDAR\r\n";

    #[test]
    fn parses_utc_floating_all_day_folding_and_escapes() {
        let ics = format!(
            "{HEADER}BEGIN:VEVENT\r\nUID:utc\r\nDTSTAMP:20260101T000000Z\r\nDTSTART:20261001T010000Z\r\nDTEND:20261001T020000Z\r\nSUMMARY:Google event\r\nDESCRIPTION:line 1\\nline 2\\, ok\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:floating\r\nDTSTAMP:20260101T000000Z\r\nDTSTART:20261002T100000\r\nSUMMARY:Apple long title that is fol\r\n ded\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:all-day\r\nDTSTAMP:20260101T000000Z\r\nDTSTART;VALUE=DATE:20261003\r\nDTEND;VALUE=DATE:20261005\r\nSUMMARY:All day\r\nEND:VEVENT\r\n{FOOTER}"
        );
        let parsed = parse(&ics).unwrap();
        assert_eq!(parsed.events.len(), 3);
        assert_eq!(
            parsed.events[0].event.start_at.to_string(),
            "2026-10-01 10:00:00"
        );
        assert_eq!(
            parsed.events[0].event.description.as_deref(),
            Some("line 1\nline 2, ok")
        );
        assert_eq!(
            parsed.events[1].event.start_at.to_string(),
            "2026-10-02 10:00:00"
        );
        assert_eq!(
            parsed.events[2].event.end_at.date().to_string(),
            "2026-10-04"
        );
    }

    #[test]
    fn parses_outlook_timezone_and_supported_recurrence() {
        let ics = format!(
            "{HEADER}BEGIN:VEVENT\r\nUID:outlook\r\nDTSTAMP:20260101T000000Z\r\nDTSTART;TZID=Tokyo Standard Time:20261005T190000\r\nDTEND;TZID=Tokyo Standard Time:20261005T200000\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE;COUNT=4\r\nSUMMARY:Outlook event\r\nEND:VEVENT\r\n{FOOTER}"
        );
        let parsed = parse(&ics).unwrap();
        let event = &parsed.events[0].event;
        assert_eq!(event.start_at.to_string(), "2026-10-05 19:00:00");
        assert!(matches!(event.recurrence, Some(Rule::Weekly { .. })));
    }

    #[test]
    fn keeps_the_last_occurrence_at_or_before_until() {
        let ics = format!(
            "{HEADER}BEGIN:VEVENT\r\nUID:until\r\nDTSTAMP:20260101T000000Z\r\nDTSTART;TZID=Asia/Tokyo:20261001T190000\r\nRRULE:FREQ=DAILY;UNTIL=20261005T000000Z\r\nSUMMARY:Until event\r\nEND:VEVENT\r\n{FOOTER}"
        );
        let parsed = parse(&ics).unwrap();
        assert!(matches!(
            parsed.events[0].event.recurrence,
            Some(Rule::Daily {
                end: Ending::Until { date }
            }) if date == "2026-10-04".parse::<NaiveDate>().unwrap()
        ));

        let before_start = ics.replace("20261005T000000Z", "20260930T000000Z");
        let parsed = parse(&before_start).unwrap();
        assert!(parsed.events.is_empty());
        assert_eq!(parsed.skipped[0].reason, "unsupported_recurrence");
    }

    #[test]
    fn parses_iana_and_embedded_timezones() {
        let ics = format!(
            "{HEADER}BEGIN:VTIMEZONE\r\nTZID:X-CUSTOM\r\nBEGIN:STANDARD\r\nDTSTART:19700101T000000\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0100\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\nBEGIN:VEVENT\r\nUID:iana\r\nDTSTAMP:20260101T000000Z\r\nDTSTART;TZID=Asia/Tokyo:20261005T100000\r\nSUMMARY:IANA\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:custom\r\nDTSTAMP:20260101T000000Z\r\nDTSTART;TZID=X-CUSTOM:20261005T100000\r\nSUMMARY:Custom\r\nEND:VEVENT\r\n{FOOTER}"
        );
        let parsed = parse(&ics).unwrap();
        assert_eq!(
            parsed.events[0].event.start_at.to_string(),
            "2026-10-05 10:00:00"
        );
        assert_eq!(
            parsed.events[1].event.start_at.to_string(),
            "2026-10-05 18:00:00"
        );
    }

    #[test]
    fn recurring_event_matches_a_later_date_range() {
        let event = ImportEventInput {
            name: "weekly".into(),
            description: None,
            color: "#2196F3".into(),
            is_all_day: false,
            start_at: "2026-10-05T10:00:00".parse().unwrap(),
            end_at: "2026-10-05T11:00:00".parse().unwrap(),
            recurrence: Some(Rule::Weekly {
                weekdays: vec![0],
                end: Ending::Never,
            }),
        };
        assert!(
            overlaps(
                &event,
                "2026-10-12".parse().unwrap(),
                "2026-10-12".parse().unwrap()
            )
            .unwrap()
        );
        assert!(
            !overlaps(
                &event,
                "2026-10-13".parse().unwrap(),
                "2026-10-13".parse().unwrap()
            )
            .unwrap()
        );
    }

    #[test]
    fn skips_exception_series_and_unsupported_rules() {
        let ics = format!(
            "{HEADER}BEGIN:VEVENT\r\nUID:series\r\nDTSTAMP:20260101T000000Z\r\nDTSTART:20261001T100000Z\r\nRRULE:FREQ=YEARLY\r\nSUMMARY:Unsupported\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nUID:exception\r\nDTSTAMP:20260101T000000Z\r\nDTSTART:20261001T100000Z\r\nRRULE:FREQ=DAILY;COUNT=3\r\nEXDATE:20261002T100000Z\r\nSUMMARY:Exception\r\nEND:VEVENT\r\n{FOOTER}"
        );
        let parsed = parse(&ics).unwrap();
        assert!(parsed.events.is_empty());
        assert_eq!(
            parsed.skipped.iter().map(|item| item.count).sum::<usize>(),
            2
        );
    }

    #[test]
    fn enforces_file_size_and_truncates_by_characters() {
        assert!(parse(&"x".repeat(ICS_MAX_BYTES + 1)).is_err());
        let title = "予".repeat(NAME_MAX_CHARS + 1);
        let description = "説".repeat(DESCRIPTION_MAX_CHARS + 1);
        let ics = format!(
            "{HEADER}BEGIN:VEVENT\r\nUID:long\r\nDTSTAMP:20260101T000000Z\r\nDTSTART:20261001T100000\r\nSUMMARY:{title}\r\nDESCRIPTION:{description}\r\nEND:VEVENT\r\n{FOOTER}"
        );
        let parsed = parse(&ics).unwrap();
        let item = &parsed.events[0];
        assert_eq!(item.event.name.chars().count(), NAME_MAX_CHARS);
        assert_eq!(
            item.event.description.as_ref().unwrap().chars().count(),
            DESCRIPTION_MAX_CHARS
        );
        assert_eq!(item.truncated_fields, ["name", "description"]);
    }
}
