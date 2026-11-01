//! 参加時の案内 (#376) の送信記録 (`guild_welcome_messages`)。
//!
//! 同じサーバーには [`RESEND_AFTER_DAYS`] 日に 1 回までしか送らない。
//! 退出では行を消さないので、追加し直し・誤って外した直後の再参加では送らない。

use chrono::{Duration, NaiveDateTime};
use sqlx::PgPool;

/// 前回の案内からこの日数が経っていれば、再参加のときにもう一度送る
pub const RESEND_AFTER_DAYS: i64 = 30;

/// 案内を送る権利を取る。送ってよければ true (その時点で送信済みとして記録する)。
///
/// 記録と判定を 1 文で行うので、同じ参加イベントが重なって届いても送るのは 1 回だけになる
pub async fn claim(pool: &PgPool, guild_id: &str, now: NaiveDateTime) -> sqlx::Result<bool> {
    let resend_before = now - Duration::days(RESEND_AFTER_DAYS);
    let claimed = sqlx::query_scalar!(
        r#"
        INSERT INTO guild_welcome_messages (guild_id, sent_at) VALUES ($1, $2)
        ON CONFLICT (guild_id) DO UPDATE SET sent_at = EXCLUDED.sent_at
            WHERE guild_welcome_messages.sent_at <= $3
        RETURNING guild_id
        "#,
        guild_id,
        now,
        resend_before,
    )
    .fetch_optional(pool)
    .await?;
    Ok(claimed.is_some())
}

/// 送れなかったときに記録を取り消す (次に参加したときに送り直せるようにする)。
/// 取り消すのは今回の記録 (`sent_at = now`) だけで、他の参加で書いた記録は残す
pub async fn release(pool: &PgPool, guild_id: &str, now: NaiveDateTime) -> sqlx::Result<()> {
    sqlx::query!(
        "DELETE FROM guild_welcome_messages WHERE guild_id = $1 AND sent_at = $2",
        guild_id,
        now,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// 保存済みのサーバー言語 (`guild_config.locale`)。web の設定で選んでいなければ None
pub async fn saved_locale(pool: &PgPool, guild_id: &str) -> sqlx::Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT locale FROM guild_config WHERE guild_id = $1",
        guild_id
    )
    .fetch_optional(pool)
    .await?
    .flatten())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> NaiveDateTime {
        s.parse().unwrap()
    }

    #[sqlx::test(migrations = "../api/migrations")]
    async fn claims_once_until_the_resend_interval(pool: PgPool) {
        let first = at("2026-11-01T12:00:00");
        assert!(claim(&pool, "1", first).await.unwrap());
        // 退出直後の再参加・同じイベントの重複では送らない
        assert!(!claim(&pool, "1", first).await.unwrap());
        assert!(!claim(&pool, "1", at("2026-11-30T12:00:00")).await.unwrap());
        // 別のサーバーは関係ない
        assert!(claim(&pool, "2", first).await.unwrap());
        // 30 日経てば送る
        assert!(claim(&pool, "1", at("2026-12-01T12:00:00")).await.unwrap());
    }

    #[sqlx::test(migrations = "../api/migrations")]
    async fn release_only_removes_its_own_claim(pool: PgPool) {
        let now = at("2026-11-01T12:00:00");
        assert!(claim(&pool, "1", now).await.unwrap());
        // 別の時刻の取り消しでは消えない
        release(&pool, "1", at("2026-11-01T12:00:01"))
            .await
            .unwrap();
        assert!(!claim(&pool, "1", now).await.unwrap());
        release(&pool, "1", now).await.unwrap();
        assert!(claim(&pool, "1", now).await.unwrap());
    }

    #[sqlx::test(migrations = "../api/migrations")]
    async fn saved_locale_is_none_without_a_choice(pool: PgPool) {
        assert_eq!(saved_locale(&pool, "1").await.unwrap(), None);
        sqlx::query("INSERT INTO guild_config (guild_id) VALUES ('1')")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(saved_locale(&pool, "1").await.unwrap(), None);
        sqlx::query("UPDATE guild_config SET locale = 'en' WHERE guild_id = '1'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            saved_locale(&pool, "1").await.unwrap().as_deref(),
            Some("en")
        );
    }
}
