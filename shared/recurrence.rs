//! APIとBotで同じ開催日を計算する。公開する条件だけからRRULEを組み立てる。
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};

pub const LOOKAHEAD_DAYS: i64 = 730;
pub const LOOKBACK_DAYS: i64 = 366;
const MAX_RESULTS: u16 = 65535;
const DAYS: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "frequency", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rule {
    None,
    Daily { end: Ending },
    Weekly { weekdays: Vec<u8>, end: Ending },
    Biweekly { weekdays: Vec<u8>, end: Ending },
    MonthlyDate { day: u8, end: Ending },
    MonthlyWeekday { nth: u8, weekday: u8, end: Ending },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Ending {
    Never,
    Until { date: NaiveDate },
    Count { count: u32 },
}

impl Rule {
    pub fn enabled(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn ending_mut(&mut self) -> Option<&mut Ending> {
        match self {
            Self::None => None,
            Self::Daily { end }
            | Self::Weekly { end, .. }
            | Self::Biweekly { end, .. }
            | Self::MonthlyDate { end, .. }
            | Self::MonthlyWeekday { end, .. } => Some(end),
        }
    }

    pub fn rrule(&self, start: NaiveDateTime) -> Result<String, String> {
        if start.nanosecond() != 0 {
            return Err("繰り返し予定の開始日時は秒単位で指定してください".into());
        }
        let weekday = start.weekday().num_days_from_monday() as u8;
        let (mut rule, ending) = match self {
            Self::None => return Err("繰り返し条件を指定してください".into()),
            Self::Daily { end } => ("FREQ=DAILY".to_owned(), end),
            Self::Weekly { weekdays, end } | Self::Biweekly { weekdays, end } => {
                if weekdays.is_empty()
                    || weekdays.len() > 7
                    || weekdays.iter().any(|d| *d > 6)
                    || !weekdays.contains(&weekday)
                {
                    return Err("開始日の曜日を含む曜日を選択してください".into());
                }
                let mut days = weekdays.clone();
                days.sort_unstable();
                days.dedup();
                let byday = days
                    .iter()
                    .map(|d| DAYS[*d as usize])
                    .collect::<Vec<_>>()
                    .join(",");
                (
                    format!(
                        "FREQ=WEEKLY;INTERVAL={};WKST=MO;BYDAY={byday}",
                        if matches!(self, Self::Biweekly { .. }) {
                            2
                        } else {
                            1
                        }
                    ),
                    end,
                )
            }
            Self::MonthlyDate { day, end } => {
                if !(1..=31).contains(day) || u32::from(*day) != start.day() {
                    return Err("開始日と同じ日付を指定してください".into());
                }
                (format!("FREQ=MONTHLY;BYMONTHDAY={day}"), end)
            }
            Self::MonthlyWeekday {
                nth,
                weekday: day,
                end,
            } => {
                if !(1..=5).contains(nth)
                    || *day > 6
                    || *day != weekday
                    || u32::from(*nth) != (start.day() - 1) / 7 + 1
                {
                    return Err("開始日と同じ第n曜日を指定してください".into());
                }
                (
                    format!("FREQ=MONTHLY;BYDAY={}{}", nth, DAYS[*day as usize]),
                    end,
                )
            }
        };
        match ending {
            Ending::Never => (),
            Ending::Count { count } => {
                if !(1..=10000).contains(count) {
                    return Err("回数は1〜10000回で指定してください".into());
                }
                rule.push_str(&format!(";COUNT={count}"));
            }
            Ending::Until { date } => {
                if *date < start.date() {
                    return Err("繰り返しの終了日は開始日以降にしてください".into());
                }
                let end = date.and_hms_opt(23, 59, 59).ok_or("終了日が不正です")?;
                let utc = end
                    .checked_sub_signed(Duration::hours(9))
                    .ok_or("終了日が不正です")?;
                rule.push_str(&format!(";UNTIL={}Z", utc.format("%Y%m%dT%H%M%S")));
            }
        }
        Ok(rule)
    }
}

fn set(rule: &str, start: NaiveDateTime) -> Result<rrule::RRuleSet, String> {
    format!(
        "DTSTART;TZID=Asia/Tokyo:{}\nRRULE:{rule}",
        start.format("%Y%m%dT%H%M%S")
    )
    .parse()
    .map_err(|_| "繰り返し条件を解釈できません".into())
}

/// 開始を含み終了を含まない範囲。上限で欠けた一覧を正常な結果にしない。
pub fn between(
    rule: &str,
    start: NaiveDateTime,
    from: NaiveDateTime,
    to: NaiveDateTime,
) -> Result<Vec<NaiveDateTime>, String> {
    if to <= from {
        return Ok(Vec::new());
    }
    let zone = rrule::Tz::Asia__Tokyo;
    let after = zone
        .from_local_datetime(&from)
        .single()
        .ok_or("開始範囲が不正です")?;
    let before = zone
        .from_local_datetime(&to)
        .single()
        .ok_or("終了範囲が不正です")?;
    let result = set(rule, start)?
        .after(after)
        .before(before)
        .all(MAX_RESULTS);
    if result.limited {
        return Err("繰り返しの計算上限に達しました。期間を短くしてください".into());
    }
    Ok(result
        .dates
        .into_iter()
        .map(|d| d.naive_local())
        .filter(|d| *d >= from && *d < to)
        .collect())
}

/// 有限ルールの最後の開催枠より後の境界。終了済みシリーズをSQLで除外するため保存する。
pub fn end_before(rule: &Rule, start: NaiveDateTime) -> Result<Option<NaiveDateTime>, String> {
    let mut rule = rule.clone();
    match rule.ending_mut().cloned() {
        Some(Ending::Count { count }) => {
            let dates = set(&rule.rrule(start)?, start)?.all(MAX_RESULTS);
            if dates.limited || dates.dates.len() != count as usize {
                return Err("繰り返しの計算上限に達しました".into());
            }
            dates
                .dates
                .last()
                .unwrap()
                .naive_local()
                .checked_add_signed(Duration::seconds(1))
                .map(Some)
                .ok_or_else(|| "終了日が範囲外です".into())
        }
        Some(Ending::Until { date }) => date
            .succ_opt()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(Some)
            .ok_or_else(|| "終了日が範囲外です".into()),
        _ => Ok(None),
    }
}

pub fn preview(rule: &Rule, start: NaiveDateTime) -> Result<Vec<NaiveDateTime>, String> {
    let text = rule.rrule(start)?;
    let result = set(&text, start)?.all(3);
    if result.limited && result.dates.len() < 3 {
        return Err("繰り返しの計算上限に達しました".into());
    }
    Ok(result.dates.into_iter().map(|d| d.naive_local()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dt(s: &str) -> NaiveDateTime {
        s.parse().unwrap()
    }
    #[test]
    fn calendar_boundaries_and_endings() {
        let start = dt("2026-01-31T19:30:00");
        let rule = Rule::MonthlyDate {
            day: 31,
            end: Ending::Count { count: 3 },
        };
        assert_eq!(
            preview(&rule, start).unwrap(),
            vec![start, dt("2026-03-31T19:30:00"), dt("2026-05-31T19:30:00")]
        );
        let fifth = Rule::MonthlyWeekday {
            nth: 5,
            weekday: 0,
            end: Ending::Never,
        };
        assert_eq!(
            preview(&fifth, dt("2026-03-30T00:00:00")).unwrap()[1],
            dt("2026-06-29T00:00:00")
        );
        let weekly = Rule::Biweekly {
            weekdays: vec![0, 2],
            end: Ending::Until {
                date: "2027-01-11".parse().unwrap(),
            },
        };
        let start = dt("2026-12-28T19:30:00");
        assert_eq!(
            between(
                &weekly.rrule(start).unwrap(),
                start,
                start,
                dt("2027-02-01T00:00:00")
            )
            .unwrap(),
            vec![start, dt("2026-12-30T19:30:00"), dt("2027-01-11T19:30:00")]
        );
        let leap = Rule::MonthlyDate {
            day: 29,
            end: Ending::Never,
        };
        assert_eq!(
            preview(&leap, dt("2028-01-29T00:00:00")).unwrap()[1],
            dt("2028-02-29T00:00:00")
        );
        assert!(weekly.rrule(dt("2026-12-29T19:30:00")).is_err());
        assert!(weekly.rrule(dt("2026-12-28T19:30:00.123")).is_err());
    }
}
