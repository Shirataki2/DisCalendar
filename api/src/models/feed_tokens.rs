//! iCal フィードのトークン (`guild_feed_tokens` テーブル、#95)。
//!
//! 1 ギルド 1 本。`GET /feeds/<token>.ics` (認証なし) がこの値でギルドを引き、予定を iCalendar 形式で配信する。
//! URL を知っている人は誰でも読めるので、推測できない長さの乱数にし、漏れたときは再発行で置き換える。
//! 平文で保存するのは、発行後もサーバー設定でいつでも URL を表示・コピーできるようにするため
//! (マイグレーションのコメントを参照)

use std::fmt::Write as _;

use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::{PgExecutor, PgPool};
use utoipa::ToSchema;

use super::guilds::Guild;

/// トークンの長さ (32 バイトの乱数の 16 進表記)
pub const TOKEN_LEN: usize = 64;

/// 発行済みのフィード。API のレスポンスにもそのまま使う
#[derive(Debug, Serialize, ToSchema)]
pub struct FeedToken {
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    /// フィード URL のパスに入る値 (`/feeds/<token>.ics`)。web が自分のオリジンと組み合わせて URL を作る
    #[schema(example = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")]
    pub token: String,
    /// 発行日時 (タイムゾーンなしの JST)
    #[schema(example = "2026-09-03T23:55:36")]
    pub created_at: NaiveDateTime,
    /// 発行した Discord ユーザー ID
    #[schema(example = "123456789012345678")]
    pub created_by: String,
}

/// 新しいトークンを作る (OS の CSPRNG 由来の 32 バイトを小文字の 16 進にする)
pub fn generate_token() -> String {
    let bytes: [u8; 32] = rand::random();
    let mut token = String::with_capacity(TOKEN_LEN);
    for byte in bytes {
        write!(token, "{byte:02x}").expect("writing to a String never fails");
    }
    token
}

/// [`generate_token`] が作る形 (64 文字の小文字 16 進) か。
/// 配信側はこれを通らない値を DB に問い合わせず 404 にする (存在の有無を区別しない)
pub fn is_token(value: &str) -> bool {
    value.len() == TOKEN_LEN
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// ギルドの発行済みフィード。未発行なら `None`
pub async fn get(pool: &PgPool, guild_id: &str) -> sqlx::Result<Option<FeedToken>> {
    sqlx::query_as!(
        FeedToken,
        "SELECT guild_id, token, created_at, created_by FROM guild_feed_tokens WHERE guild_id = $1",
        guild_id
    )
    .fetch_optional(pool)
    .await
}

/// 発行・再発行。既に行があれば置き換える (古いトークンはその時点で失効する)
pub async fn upsert<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    token: &str,
    created_by: &str,
    created_at: NaiveDateTime,
) -> sqlx::Result<FeedToken> {
    sqlx::query_as!(
        FeedToken,
        r#"
        INSERT INTO guild_feed_tokens (guild_id, token, created_at, created_by)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (guild_id) DO UPDATE
            SET token = EXCLUDED.token, created_at = EXCLUDED.created_at, created_by = EXCLUDED.created_by
        RETURNING guild_id, token, created_at, created_by
        "#,
        guild_id,
        token,
        created_at,
        created_by
    )
    .fetch_one(executor)
    .await
}

/// 無効化。消せたら `true` (未発行なら `false`)
pub async fn delete<'e>(executor: impl PgExecutor<'e>, guild_id: &str) -> sqlx::Result<bool> {
    let result = sqlx::query!(
        "DELETE FROM guild_feed_tokens WHERE guild_id = $1",
        guild_id
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// 配信用: トークンからギルドを引く。
/// `guilds` (Bot が参加中のギルド) と JOIN するので、Bot が退出したギルドのトークンは発行済みでも `None`
/// (通常 API の「Bot 未参加なら 403」に相当。退出後も URL だけで予定が読め続けないようにする)
pub async fn find_guild_by_token(pool: &PgPool, token: &str) -> sqlx::Result<Option<Guild>> {
    sqlx::query_as!(
        Guild,
        r#"
        SELECT g.guild_id, g.name, g.avatar_url, g.locale
        FROM guild_feed_tokens t
        JOIN guilds g ON g.guild_id = t.guild_id
        WHERE t.token = $1
        "#,
        token
    )
    .fetch_optional(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_are_valid_and_distinct() {
        let a = generate_token();
        let b = generate_token();
        assert!(is_token(&a), "{a}");
        assert!(is_token(&b), "{b}");
        assert_ne!(a, b);
    }

    #[test]
    fn token_validation() {
        let valid = "0".repeat(TOKEN_LEN);
        assert!(is_token(&valid));
        assert!(!is_token(""));
        assert!(!is_token(&"0".repeat(TOKEN_LEN - 1)));
        assert!(!is_token(&"0".repeat(TOKEN_LEN + 1)));
        // 大文字・記号・非 ASCII は通さない (DB に問い合わせる前に弾く)
        assert!(!is_token(&format!("A{}", "0".repeat(TOKEN_LEN - 1))));
        assert!(!is_token(&format!("/{}", "0".repeat(TOKEN_LEN - 1))));
        assert!(!is_token(&format!("あ{}", "0".repeat(TOKEN_LEN - 3))));
    }
}
