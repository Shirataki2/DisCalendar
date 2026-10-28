//! APIとBotで同じ開催日を計算する。公開する条件だけからRRULEを組み立てる。
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};

pub const LOOKAHEAD_DAYS: i64 = 730;
pub const LOOKBACK_DAYS: i64 = 366;
const MAX_RESULTS: u16 = 65535;
const DAYS: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];

/// 毎月は日付・第n曜日とも複数指定できる。第n曜日は週の順番と曜日の全組合せで開催する。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "frequency", rename_all = "snake_case", try_from = "RuleInput")]
pub enum Rule {
    None,
    Daily {
        end: Ending,
    },
    Yearly {
        end: Ending,
    },
    Weekly {
        weekdays: Vec<u8>,
        end: Ending,
    },
    Biweekly {
        weekdays: Vec<u8>,
        end: Ending,
    },
    MonthlyDate {
        days: Vec<u8>,
        end: Ending,
    },
    MonthlyWeekday {
        nths: Vec<u8>,
        weekdays: Vec<u8>,
        end: Ending,
    },
}

/// 入力と保存済みJSONの読み込み用。#355以前の単一値 (`day` / `nth`+`weekday`) も受け付ける。
#[derive(Deserialize)]
#[serde(tag = "frequency", rename_all = "snake_case", deny_unknown_fields)]
enum RuleInput {
    None,
    Daily {
        end: Ending,
    },
    Yearly {
        end: Ending,
    },
    Weekly {
        weekdays: Vec<u8>,
        end: Ending,
    },
    Biweekly {
        weekdays: Vec<u8>,
        end: Ending,
    },
    MonthlyDate {
        #[serde(default)]
        days: Option<Vec<u8>>,
        #[serde(default)]
        day: Option<u8>,
        end: Ending,
    },
    MonthlyWeekday {
        #[serde(default)]
        nths: Option<Vec<u8>>,
        #[serde(default)]
        weekdays: Option<Vec<u8>>,
        #[serde(default)]
        nth: Option<u8>,
        #[serde(default)]
        weekday: Option<u8>,
        end: Ending,
    },
}

impl TryFrom<RuleInput> for Rule {
    type Error = String;

    fn try_from(input: RuleInput) -> Result<Self, String> {
        Ok(match input {
            RuleInput::None => Self::None,
            RuleInput::Daily { end } => Self::Daily { end },
            RuleInput::Yearly { end } => Self::Yearly { end },
            RuleInput::Weekly { weekdays, end } => Self::Weekly { weekdays, end },
            RuleInput::Biweekly { weekdays, end } => Self::Biweekly { weekdays, end },
            RuleInput::MonthlyDate { days, day, end } => Self::MonthlyDate {
                days: match (days, day) {
                    (Some(days), None) => days,
                    (None, Some(day)) => vec![day],
                    _ => return Err("毎月の日付は days で指定してください".into()),
                },
                end,
            },
            RuleInput::MonthlyWeekday {
                nths,
                weekdays,
                nth,
                weekday,
                end,
            } => match (nths, weekdays, nth, weekday) {
                (Some(nths), Some(weekdays), None, None) => Self::MonthlyWeekday {
                    nths,
                    weekdays,
                    end,
                },
                (None, None, Some(nth), Some(weekday)) => Self::MonthlyWeekday {
                    nths: vec![nth],
                    weekdays: vec![weekday],
                    end,
                },
                _ => return Err("毎月の第n曜日は nths と weekdays で指定してください".into()),
            },
        })
    }
}

/// 重複を除いて昇順に並べる。空・範囲外は呼び出し側で弾く。
fn normalized(values: &[u8]) -> Vec<u8> {
    let mut values = values.to_vec();
    values.sort_unstable();
    values.dedup();
    values
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
            | Self::Yearly { end }
            | Self::Weekly { end, .. }
            | Self::Biweekly { end, .. }
            | Self::MonthlyDate { end, .. }
            | Self::MonthlyWeekday { end, .. } => Some(end),
        }
    }

    pub fn ending(&self) -> Option<&Ending> {
        match self {
            Self::None => None,
            Self::Daily { end }
            | Self::Yearly { end }
            | Self::Weekly { end, .. }
            | Self::Biweekly { end, .. }
            | Self::MonthlyDate { end, .. }
            | Self::MonthlyWeekday { end, .. } => Some(end),
        }
    }

    /// 曜日・日付の配列を重複なしの昇順にそろえる。保存するJSONとRRULEを一意にするため。
    pub fn normalize(&mut self) {
        match self {
            Self::Weekly { weekdays, .. } | Self::Biweekly { weekdays, .. } => {
                *weekdays = normalized(weekdays);
            }
            Self::MonthlyDate { days, .. } => *days = normalized(days),
            Self::MonthlyWeekday { nths, weekdays, .. } => {
                *nths = normalized(nths);
                *weekdays = normalized(weekdays);
            }
            _ => (),
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
            Self::Yearly { end } => ("FREQ=YEARLY".to_owned(), end),
            Self::Weekly { weekdays, end } | Self::Biweekly { weekdays, end } => {
                if weekdays.is_empty()
                    || weekdays.len() > 7
                    || weekdays.iter().any(|d| *d > 6)
                    || !weekdays.contains(&weekday)
                {
                    return Err("開始日の曜日を含む曜日を選択してください".into());
                }
                let byday = normalized(weekdays)
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
            Self::MonthlyDate { days, end } => {
                if days.is_empty()
                    || days.iter().any(|d| !(1..=31).contains(d))
                    || !days.iter().any(|d| u32::from(*d) == start.day())
                {
                    return Err("開始日の日付を含む日付を選択してください".into());
                }
                let bymonthday = normalized(days)
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                (format!("FREQ=MONTHLY;BYMONTHDAY={bymonthday}"), end)
            }
            Self::MonthlyWeekday {
                nths,
                weekdays,
                end,
            } => {
                let nth = (start.day() - 1) / 7 + 1;
                if nths.is_empty()
                    || weekdays.is_empty()
                    || nths.iter().any(|n| !(1..=5).contains(n))
                    || weekdays.iter().any(|d| *d > 6)
                    || !nths.iter().any(|n| u32::from(*n) == nth)
                    || !weekdays.contains(&weekday)
                {
                    return Err("開始日の第n曜日を含む週の順番と曜日を選択してください".into());
                }
                let nths = normalized(nths);
                let byday = normalized(weekdays)
                    .iter()
                    .flat_map(|d| {
                        nths.iter()
                            .map(move |n| format!("{n}{}", DAYS[*d as usize]))
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                (format!("FREQ=MONTHLY;BYDAY={byday}"), end)
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
    fn yearly_endings_leap_days_and_generation_window() {
        let start = dt("2026-10-09T19:30:00");
        let rule: Rule =
            serde_json::from_str(r#"{"frequency":"yearly","end":{"type":"never"}}"#).unwrap();
        assert_eq!(serde_json::to_value(&rule).unwrap()["frequency"], "yearly");
        assert_eq!(
            preview(&rule, start).unwrap(),
            vec![start, dt("2027-10-09T19:30:00"), dt("2028-10-09T19:30:00")]
        );
        let window = between(
            &rule.rrule(start).unwrap(),
            start,
            start,
            start + Duration::days(LOOKAHEAD_DAYS),
        )
        .unwrap();
        assert_eq!(window, vec![start, dt("2027-10-09T19:30:00")]);
        let leap = dt("2028-02-29T10:00:00");
        assert_eq!(
            preview(&rule, leap).unwrap(),
            vec![leap, dt("2032-02-29T10:00:00"), dt("2036-02-29T10:00:00")]
        );
        assert!(
            between(
                &rule.rrule(leap).unwrap(),
                leap,
                leap + Duration::days(1),
                leap + Duration::days(LOOKAHEAD_DAYS)
            )
            .unwrap()
            .is_empty()
        );
        let count = Rule::Yearly {
            end: Ending::Count { count: 2 },
        };
        assert_eq!(
            preview(&count, leap).unwrap(),
            vec![leap, dt("2032-02-29T10:00:00")]
        );
        assert_eq!(
            end_before(&count, leap).unwrap(),
            Some(dt("2032-02-29T10:00:01"))
        );
        let until = Rule::Yearly {
            end: Ending::Until {
                date: "2027-10-09".parse().unwrap(),
            },
        };
        assert_eq!(preview(&until, start).unwrap(), window);
        assert!(
            Rule::Yearly {
                end: Ending::Count { count: 0 }
            }
            .rrule(start)
            .is_err()
        );
    }

    #[test]
    fn calendar_boundaries_and_endings() {
        let start = dt("2026-01-31T19:30:00");
        let rule = Rule::MonthlyDate {
            days: vec![31],
            end: Ending::Count { count: 3 },
        };
        assert_eq!(
            preview(&rule, start).unwrap(),
            vec![start, dt("2026-03-31T19:30:00"), dt("2026-05-31T19:30:00")]
        );
        let fifth = Rule::MonthlyWeekday {
            nths: vec![5],
            weekdays: vec![0],
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
            days: vec![29],
            end: Ending::Never,
        };
        assert_eq!(
            preview(&leap, dt("2028-01-29T00:00:00")).unwrap()[1],
            dt("2028-02-29T00:00:00")
        );
        assert!(weekly.rrule(dt("2026-12-29T19:30:00")).is_err());
        assert!(weekly.rrule(dt("2026-12-28T19:30:00.123")).is_err());
    }

    #[test]
    fn monthly_multiple_dates() {
        let start = dt("2026-01-15T19:30:00");
        let rule = Rule::MonthlyDate {
            days: vec![25, 5, 15, 31, 15],
            end: Ending::Count { count: 6 },
        };
        assert_eq!(
            rule.rrule(start).unwrap(),
            "FREQ=MONTHLY;BYMONTHDAY=5,15,25,31;COUNT=6"
        );
        // 2月は31日がないのでスキップする。
        let dates = between(
            &rule.rrule(start).unwrap(),
            start,
            start,
            dt("2027-01-01T00:00:00"),
        )
        .unwrap();
        assert_eq!(
            dates,
            [
                "2026-01-15T19:30:00",
                "2026-01-25T19:30:00",
                "2026-01-31T19:30:00",
                "2026-02-05T19:30:00",
                "2026-02-15T19:30:00",
                "2026-02-25T19:30:00",
            ]
            .map(dt)
        );
        assert_eq!(
            end_before(&rule, start).unwrap(),
            Some(dt("2026-02-25T19:30:01"))
        );
        // 開始日の日付を含まない・空・範囲外は不可。
        for days in [vec![5, 25], vec![], vec![0, 15], vec![15, 32]] {
            assert!(
                Rule::MonthlyDate {
                    days,
                    end: Ending::Never
                }
                .rrule(start)
                .is_err()
            );
        }
    }

    #[test]
    fn monthly_multiple_weekdays() {
        // 2026-11-01は第1日曜。
        let start = dt("2026-11-01T10:00:00");
        let rule = Rule::MonthlyWeekday {
            nths: vec![3, 1, 2],
            weekdays: vec![6],
            end: Ending::Never,
        };
        assert_eq!(rule.rrule(start).unwrap(), "FREQ=MONTHLY;BYDAY=1SU,2SU,3SU");
        let dates = between(
            &rule.rrule(start).unwrap(),
            start,
            start,
            dt("2026-12-31T00:00:00"),
        )
        .unwrap();
        assert_eq!(
            dates,
            [
                "2026-11-01T10:00:00",
                "2026-11-08T10:00:00",
                "2026-11-15T10:00:00",
                "2026-12-06T10:00:00",
                "2026-12-13T10:00:00",
                "2026-12-20T10:00:00",
            ]
            .map(dt)
        );
        // 第2・第4 × 土日。2026-11-14は第2土曜。
        let start = dt("2026-11-14T10:00:00");
        let rule = Rule::MonthlyWeekday {
            nths: vec![2, 4],
            weekdays: vec![5, 6],
            end: Ending::Count { count: 4 },
        };
        assert_eq!(
            rule.rrule(start).unwrap(),
            "FREQ=MONTHLY;BYDAY=2SA,4SA,2SU,4SU;COUNT=4"
        );
        assert_eq!(
            preview(&rule, start).unwrap(),
            [
                "2026-11-14T10:00:00",
                "2026-11-22T10:00:00",
                "2026-11-28T10:00:00"
            ]
            .map(dt)
        );
        for (nths, weekdays) in [
            (vec![1, 4], vec![5]),
            (vec![2], vec![6]),
            (vec![], vec![5]),
            (vec![2], vec![]),
            (vec![2, 6], vec![5]),
            (vec![2], vec![5, 7]),
        ] {
            assert!(
                Rule::MonthlyWeekday {
                    nths,
                    weekdays,
                    end: Ending::Never
                }
                .rrule(start)
                .is_err()
            );
        }
    }

    #[test]
    fn legacy_monthly_json_is_read_as_arrays() {
        let date: Rule =
            serde_json::from_str(r#"{"frequency":"monthly_date","day":5,"end":{"type":"never"}}"#)
                .unwrap();
        assert_eq!(
            date,
            Rule::MonthlyDate {
                days: vec![5],
                end: Ending::Never
            }
        );
        assert_eq!(
            serde_json::to_value(&date).unwrap(),
            serde_json::json!({"frequency":"monthly_date","days":[5],"end":{"type":"never"}})
        );
        let weekday: Rule = serde_json::from_str(
            r#"{"frequency":"monthly_weekday","nth":2,"weekday":6,"end":{"type":"count","count":3}}"#,
        )
        .unwrap();
        assert_eq!(
            weekday,
            Rule::MonthlyWeekday {
                nths: vec![2],
                weekdays: vec![6],
                end: Ending::Count { count: 3 }
            }
        );
        assert_eq!(
            serde_json::to_value(&weekday).unwrap(),
            serde_json::json!({"frequency":"monthly_weekday","nths":[2],"weekdays":[6],"end":{"type":"count","count":3}})
        );
        // 新旧の混在・不足・未知の項目は受け付けない。
        for json in [
            r#"{"frequency":"monthly_date","day":5,"days":[5],"end":{"type":"never"}}"#,
            r#"{"frequency":"monthly_date","end":{"type":"never"}}"#,
            r#"{"frequency":"monthly_weekday","nth":2,"weekdays":[6],"end":{"type":"never"}}"#,
            r#"{"frequency":"monthly_weekday","nths":[2],"end":{"type":"never"}}"#,
            r#"{"frequency":"daily","day":5,"end":{"type":"never"}}"#,
        ] {
            assert!(serde_json::from_str::<Rule>(json).is_err(), "{json}");
        }
        let mut rule = Rule::MonthlyWeekday {
            nths: vec![3, 1, 3],
            weekdays: vec![6, 5],
            end: Ending::Never,
        };
        rule.normalize();
        assert_eq!(
            rule,
            Rule::MonthlyWeekday {
                nths: vec![1, 3],
                weekdays: vec![5, 6],
                end: Ending::Never
            }
        );
    }
}
