//! 利用者の日次アクティビティ (`user_daily_activity`、#81)。
//!
//! api がセッションを検証したリクエスト (`crate::auth`) を「その利用者がその日 (JST) に使った」として
//! 1 人 1 日 1 行で積み上げる。分析情報 (#79) のアクティブユーザーはセッションの生存期間からの
//! 推定だったが、この記録で正確な DAU / WAU / MAU が出せる (`admin_analytics::measured_active_users`)。
//!
//! - 記録するのは利用者 ID と日付の組だけ。IP アドレス・ユーザーエージェント・操作の内容は持たない
//! - 運営者の閲覧が指標に混ざらないよう、管理コンソール (`/admin/*`) へのリクエストは数えない
//! - 同じ日の 2 回目以降の書き込みは [`crate::state::AppState::activity_days`] のキャッシュでスキップする

use chrono::NaiveDate;
use sqlx::{Connection as _, PgConnection, PgExecutor};

/// その利用者がその日に使ったことを記録する (1 人 1 日 1 行。既にあれば何もしない)
pub async fn record<'e>(
    executor: impl PgExecutor<'e>,
    user_id: &str,
    day: NaiveDate,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO user_daily_activity (user_id, day) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        user_id,
        day,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// `"user"` への外部キー (ON DELETE CASCADE。利用者に関する情報の削除で記録も消すため) が
/// 無ければ張る。起動時 (`crate::run_startup_migrations`) に毎回呼ぶ。
///
/// `"user"` は Better Auth (web 側) が作るテーブルなので、新規環境では api のマイグレーションが
/// 先に走って migration 側の DO ブロックが外部キーを張れない (compose では api が web より先に
/// 起動する)。マイグレーションは一度成功すると再実行されないため、ここで毎回確かめて、
/// `"user"` ができるまで起動後も再試行する。トランザクションのアドバイザリロックで
/// 直列化し、未準備・他インスタンスが処理中なら false を返す。
///
/// 外部キーが無い間に `"user"` の行が消されて残った記録 (孤児) があると ADD CONSTRAINT が
/// 検証で失敗するので、同じトランザクションで先に消してから張る (削除方針の遅れた実施)
pub async fn ensure_user_fk(conn: &mut PgConnection) -> sqlx::Result<bool> {
    let mut tx = conn.begin().await?;
    let ready: bool = sqlx::query_scalar("SELECT to_regclass('public.\"user\"') IS NOT NULL AND pg_try_advisory_xact_lock(hashtext('discalendar:user-fks'))")
        .fetch_one(&mut *tx).await?;
    if !ready {
        return Ok(false);
    }
    for (table, constraint) in [
        ("user_daily_activity", "user_daily_activity_user_id_fkey"),
        ("user_push_settings", "user_push_settings_user_id_fkey"),
        ("push_subscriptions", "push_subscriptions_user_id_fkey"),
    ] {
        ensure_fk(&mut tx, table, constraint).await?;
    }
    tx.commit().await?;
    Ok(true)
}

// 識別子は上の定数だけから渡す。利用者入力は受け取らない。
async fn ensure_fk(conn: &mut PgConnection, table: &str, constraint: &str) -> sqlx::Result<()> {
    let missing: bool = sqlx::query_scalar(
        r#"
        SELECT to_regclass('public."user"') IS NOT NULL
           AND NOT EXISTS (
               SELECT 1 FROM pg_constraint
               WHERE conname = $1
                 AND conrelid = to_regclass($2)
           )
        "#,
    )
    .bind(constraint)
    .bind(format!("public.{table}"))
    .fetch_one(&mut *conn)
    .await?;
    if !missing {
        return Ok(());
    }

    let mut tx = conn.begin().await?;
    let orphans = sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM {table} t WHERE NOT EXISTS (SELECT 1 FROM \"user\" u WHERE u.id = t.user_id)"
    )))
    .execute(&mut *tx)
    .await?
    .rows_affected();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "ALTER TABLE {table} ADD CONSTRAINT {constraint} FOREIGN KEY (user_id) REFERENCES \"user\" (id) ON DELETE CASCADE"
    ))).execute(&mut *tx).await?;
    tx.commit().await?;
    tracing::info!(orphans, table, "added a missing user foreign key");
    Ok(())
}

/// web が後から作る利用者テーブルを待ち、API を再起動せず外部キーを張り終える。
pub async fn complete_user_fks(pool: sqlx::PgPool) {
    loop {
        let result = async {
            let mut conn = pool.acquire().await?;
            ensure_user_fk(&mut conn).await
        }
        .await;
        match result {
            Ok(true) => return,
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(error = %error, "failed to complete user foreign keys; retrying")
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
