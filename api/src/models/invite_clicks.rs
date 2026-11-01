//! 招待リンクの導入元ごとのクリック数 (`invite_clicks`、#374)。
//!
//! web の `/invite?from=<導入元>` (Bot の招待 URL へのリダイレクト) が開かれるたびに、
//! 導入元 × 日 (JST) の件数を 1 増やす。管理コンソールの分析情報で導入元ごとに比べる。
//!
//! - 記録するのは導入元と日付と件数だけ。IP アドレス・cookie・Discord の ID は持たない
//! - 導入元は [`KNOWN_SOURCES`] の値だけを受け付け、それ以外は [`OTHER`] にまとめる
//!   (任意の文字列を DB に溜めないため)。導入元を増やすときはここに足す
//! - 数えるのはリンクのクリック (招待画面を開いた回数) で、実際に Bot が導入されたかは分からない

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

/// 受け付ける導入元。web のリンク (`web/src/lib/site.ts` の `InviteSource`) と外部の掲載先で使う
pub const KNOWN_SOURCES: &[&str] = &[
    // サイト内 (web)
    "lp-hero",
    "lp-footer",
    "docs-start",
    "docs-invite",
    "tutorial",
    // サイト外 (Bot 掲載サイト・SNS・Bot のコマンドなど)
    "topgg",
    "discordbotsgg",
    "x",
    "bot",
    // `from` なし (ブックマーク・以前から貼られているリンクなど)
    DIRECT,
];

/// `from` が付いていないとき
pub const DIRECT: &str = "direct";
/// [`KNOWN_SOURCES`] に無い値
pub const OTHER: &str = "other";

/// 導入元の値を、記録する値に揃える (無ければ `direct`、知らない値は `other`)
pub fn normalize(source: Option<&str>) -> &'static str {
    match source.map(str::trim) {
        None | Some("") => DIRECT,
        Some(s) => KNOWN_SOURCES
            .iter()
            .find(|known| **known == s)
            .copied()
            .unwrap_or(OTHER),
    }
}

/// その日のその導入元のクリック数を 1 増やす。`source` は [`normalize`] 済みの値を渡す
pub async fn record<'e>(
    executor: impl PgExecutor<'e>,
    source: &str,
    day: NaiveDate,
) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO invite_clicks (day, source, clicks) VALUES ($1, $2, 1)
        ON CONFLICT (day, source) DO UPDATE SET clicks = invite_clicks.clicks + 1
        "#,
        day,
        source,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// 導入元ごとのクリック数
#[derive(Debug, Serialize, ToSchema)]
pub struct InviteSourceClicks {
    #[schema(example = "lp-hero")]
    pub source: String,
    /// 直近 30 日 (今日を含む)
    #[schema(example = 42)]
    pub recent: i64,
    /// その前の 30 日
    #[schema(example = 30)]
    pub previous: i64,
    /// 記録を始めてからの合計
    #[schema(example = 120)]
    pub total: i64,
}

/// 導入元ごとのクリック数の集計
#[derive(Debug, Serialize, ToSchema)]
pub struct InviteClicks {
    /// 記録が始まった日 (JST)。まだ 1 件も無ければ null
    #[schema(example = "2026-11-01")]
    pub since: Option<NaiveDate>,
    /// 直近 30 日の多い順 (同数なら合計の多い順、導入元の名前順)
    pub sources: Vec<InviteSourceClicks>,
}

/// 導入元ごとのクリック数を `today` (JST) 基準で集計する。今日より後の行は数えない
pub async fn summary<'e>(
    executor: impl PgExecutor<'e>,
    today: NaiveDate,
    recent_days: i64,
) -> sqlx::Result<InviteClicks> {
    let days = recent_days as i32;
    let rows = sqlx::query!(
        r#"
        SELECT
            source,
            min(day) AS "since!",
            COALESCE(sum(clicks) FILTER (WHERE day > $1::date - $2::int), 0)::bigint AS "recent!",
            COALESCE(sum(clicks) FILTER (
                WHERE day > $1::date - 2 * $2::int AND day <= $1::date - $2::int), 0)::bigint
                AS "previous!",
            sum(clicks)::bigint AS "total!"
        FROM invite_clicks
        WHERE day <= $1
        GROUP BY source
        ORDER BY 3 DESC, 5 DESC, source
        "#,
        today,
        days,
    )
    .fetch_all(executor)
    .await?;

    Ok(InviteClicks {
        since: rows.iter().map(|r| r.since).min(),
        sources: rows
            .into_iter()
            .map(|r| InviteSourceClicks {
                source: r.source,
                recent: r.recent,
                previous: r.previous,
                total: r.total,
            })
            .collect(),
    })
}
