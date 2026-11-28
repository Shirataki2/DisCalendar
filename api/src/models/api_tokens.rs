//! サーバー用の API トークン (#405) のモデル・検証・ログ・冪等キー・外部 ID。
//!
//! - トークン本体は `dct_` + 32 バイト乱数の小文字 16 進 (計 68 文字)。
//! - DB には平文を保存せず、SHA-256 (小文字 16 進) のみ保存する。
//! - 一覧では先頭 12 文字 (`token_prefix`) だけ返す。

use std::fmt::Write as _;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgExecutor, PgPool, Row};
use utoipa::ToSchema;

use super::feed_tokens;
use crate::error::ApiError;

/// 1 サーバーあたりの有効 (未失効) なトークン上限
pub const MAX_ACTIVE_TOKENS_PER_GUILD: i64 = 10;
/// 1 サーバーあたりのトークン総数上限 (失効済みを含む)
pub const MAX_TOTAL_TOKENS_PER_GUILD: i64 = 20;
/// トークン名の最大文字数
pub const NAME_MAX_CHARS: usize = 64;
/// 一覧 API で返すトークンごとの呼び出しログ上限
pub const LOG_LIST_LIMIT: i64 = 20;
/// DB に残すトークンごとの呼び出しログ上限
pub const LOG_RETENTION_LIMIT: i64 = 50;
/// トークン本体の接頭辞
pub const TOKEN_PREFIX: &str = "dct_";
/// 一覧表示用の先頭文字数 (`dct_` + 8 桁)
pub const DISPLAY_PREFIX_LEN: usize = 12;
/// 有効期限の最大日数 (10 年)
pub const EXPIRES_IN_DAYS_MAX: u32 = 3650;
/// `external_id` の最大長
pub const EXTERNAL_ID_MAX_LEN: usize = 128;
/// `Idempotency-Key` の最大長
pub const IDEMPOTENCY_KEY_MAX_LEN: usize = 128;
/// トークン単位の呼び出し頻度制限 (60 秒あたり)
pub const RATE_LIMIT_PER_MINUTE: u32 = 60;
/// 認証失敗時の IP + ギルド単位の頻度制限 (60 秒あたり)
pub const AUTH_FAILURE_LIMIT_PER_MINUTE: u32 = 30;
/// 冪等キーのレスポンス保持期間 (時間)
pub const IDEMPOTENCY_RETENTION_HOURS: i64 = 24;

/// API トークンの権限スコープ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApiTokenScope {
    /// 予定の作成のみ
    Create,
    /// 予定の作成・更新・削除
    Write,
}

impl ApiTokenScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Write => "write",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, ApiError> {
        match value {
            "create" => Ok(Self::Create),
            "write" => Ok(Self::Write),
            other => Err(anyhow::anyhow!("unknown api token scope: {other}").into()),
        }
    }

    pub fn can_write(self) -> bool {
        matches!(self, Self::Write)
    }
}

/// 呼び出しログ 1 件 (秘匿情報や入力値は含まない)
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct ApiTokenLog {
    pub id: i64,
    #[schema(example = "POST")]
    pub method: String,
    #[schema(example = "create")]
    pub action: String,
    pub event_id: Option<i32>,
    pub external_id: Option<String>,
    #[schema(example = 201)]
    pub status: i32,
    pub error_kind: Option<String>,
    pub called_at: DateTime<Utc>,
}

/// トークン一覧の 1 要素 (`token_hash` は含まない)
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiTokenInfo {
    #[schema(example = "1")]
    pub id: String,
    #[schema(example = "GitHub Actions 連携")]
    pub name: String,
    #[schema(example = "dct_9f86d081")]
    pub token_prefix: String,
    pub scope: ApiTokenScope,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    #[schema(example = "123456789012345678")]
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub logs: Vec<ApiTokenLog>,
}

/// 発行・ローテーション直後に 1 回だけ返すレスポンス
#[derive(Debug, Serialize, ToSchema)]
pub struct IssuedApiToken {
    #[schema(example = "1")]
    pub id: String,
    #[schema(example = "GitHub Actions 連携")]
    pub name: String,
    #[schema(example = "dct_9f86d081")]
    pub token_prefix: String,
    pub scope: ApiTokenScope,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    /// 平文トークン。このレスポンス以外では二度と返さない
    pub token: String,
}

/// Bearer 認証で確定したトークン情報
#[derive(Debug, Clone)]
pub struct AuthenticatedToken {
    pub id: i64,
    pub guild_id: String,
    pub name: String,
    pub scope: ApiTokenScope,
    pub created_by: String,
}

/// 保存済みの冪等操作
#[derive(Debug)]
pub struct SavedOperation {
    pub request_hash: String,
    pub status: i32,
    pub response_body: Option<Value>,
    pub expired: bool,
}

/// バイト列の SHA-256 を小文字 16 進で返す
pub fn hash_hex(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    let mut out = String::with_capacity(64);
    for byte in digest {
        write!(out, "{byte:02x}").expect("writing to a String never fails");
    }
    out
}

/// 新しい平文トークンを生成する (`dct_` + 64 桁小文字 16 進)
pub fn generate_token() -> String {
    format!("{TOKEN_PREFIX}{}", feed_tokens::generate_token())
}

/// 一覧識別用の先頭プレフィックスを取り出す
pub fn display_prefix(raw_token: &str) -> String {
    raw_token.chars().take(DISPLAY_PREFIX_LEN).collect()
}

/// 発行するトークン形式 (`dct_` + 64 桁小文字 16 進) に一致するか。
/// 形式に合わない入力では DB を引かずに 401 にする
pub fn is_valid_token_format(raw: &str) -> bool {
    raw.strip_prefix(TOKEN_PREFIX)
        .is_some_and(feed_tokens::is_token)
}

/// トークン名を検証してトリム済み文字列を返す
pub fn validate_name(raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::BadRequest("トークン名を入力してください".into()));
    }
    if trimmed.chars().count() > NAME_MAX_CHARS {
        return Err(ApiError::BadRequest(format!(
            "トークン名は {NAME_MAX_CHARS} 文字以内で入力してください"
        )));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(ApiError::BadRequest(
            "トークン名に制御文字は使用できません".into(),
        ));
    }
    Ok(trimmed.to_owned())
}

/// 有効期限の日数指定を検証して UTC 日時へ変換する
pub fn compute_expires_at(expires_in_days: Option<u32>) -> Result<Option<DateTime<Utc>>, ApiError> {
    let Some(days) = expires_in_days else {
        return Ok(None);
    };
    if days == 0 || days > EXPIRES_IN_DAYS_MAX {
        return Err(ApiError::BadRequest(format!(
            "有効期限は 1〜{EXPIRES_IN_DAYS_MAX} 日の範囲で指定してください"
        )));
    }
    Ok(Some(Utc::now() + Duration::days(i64::from(days))))
}

/// `external_id` を検証する (URL パスに直接入るため記号を厳しく絞る)
pub fn validate_external_id(raw: &str) -> Result<&str, ApiError> {
    if raw.is_empty() || raw.len() > EXTERNAL_ID_MAX_LEN {
        return Err(ApiError::BadRequest(format!(
            "external_id must be 1..={EXTERNAL_ID_MAX_LEN} characters"
        )));
    }
    if raw == "." || raw == ".." || raw.contains("..") {
        return Err(ApiError::BadRequest("external_id is invalid".into()));
    }
    if !raw
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':'))
    {
        return Err(ApiError::BadRequest(
            "external_id may only contain ASCII letters, digits, '.', '_', '-', and ':'".into(),
        ));
    }
    Ok(raw)
}

/// `Idempotency-Key` ヘッダを検証し、SHA-256 ハッシュを返す (MCP と同じ基準)
pub fn validate_and_hash_idempotency_key(raw: &str) -> Result<String, ApiError> {
    if raw.is_empty()
        || raw.len() > IDEMPOTENCY_KEY_MAX_LEN
        || !raw.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(ApiError::BadRequest(format!(
            "Idempotency-Key must be 1..={IDEMPOTENCY_KEY_MAX_LEN} printable ASCII characters without spaces"
        )));
    }
    Ok(hash_hex(raw.as_bytes()))
}

/// ギルドのトークン一覧と各トークンの直近ログを返す
pub async fn list_with_logs(pool: &PgPool, guild_id: &str) -> Result<Vec<ApiTokenInfo>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, name, token_prefix, scope, expires_at, last_used_at, revoked_at, created_by, created_at
         FROM guild_api_tokens
         WHERE guild_id = $1
         ORDER BY id DESC",
    )
    .bind(guild_id)
    .fetch_all(pool)
    .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let id: i64 = row.get("id");
        let scope_raw: String = row.get("scope");
        let logs: Vec<ApiTokenLog> = sqlx::query_as(
            "SELECT id, method, action, event_id, external_id, status, error_kind, called_at
             FROM guild_api_token_logs
             WHERE token_id = $1
             ORDER BY id DESC
             LIMIT $2",
        )
        .bind(id)
        .bind(LOG_LIST_LIMIT)
        .fetch_all(pool)
        .await?;

        items.push(ApiTokenInfo {
            id: id.to_string(),
            name: row.get("name"),
            token_prefix: row.get("token_prefix"),
            scope: ApiTokenScope::from_db(&scope_raw)?,
            expires_at: row.get("expires_at"),
            last_used_at: row.get("last_used_at"),
            revoked_at: row.get("revoked_at"),
            created_by: row.get("created_by"),
            created_at: row.get("created_at"),
            logs,
        });
    }
    Ok(items)
}

/// 新しいトークンを発行する (ギルド単位で直列化して上限を守る)
pub async fn create(
    pool: &PgPool,
    guild_id: &str,
    name: &str,
    scope: ApiTokenScope,
    expires_at: Option<DateTime<Utc>>,
    created_by: &str,
) -> Result<IssuedApiToken, ApiError> {
    let name = validate_name(name)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT guild_id FROM guilds WHERE guild_id = $1 FOR UPDATE")
        .bind(guild_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::NotFound("ギルドが見つかりません".into()))?;

    let counts = sqlx::query(
        "SELECT
            count(*) FILTER (WHERE revoked_at IS NULL) AS active_count,
            count(*) AS total_count
         FROM guild_api_tokens
         WHERE guild_id = $1",
    )
    .bind(guild_id)
    .fetch_one(&mut *tx)
    .await?;
    let active_count: i64 = counts.get("active_count");
    let total_count: i64 = counts.get("total_count");
    if active_count >= MAX_ACTIVE_TOKENS_PER_GUILD {
        return Err(ApiError::BadRequest(format!(
            "有効な API トークンは 1 サーバー {MAX_ACTIVE_TOKENS_PER_GUILD} 件までです"
        )));
    }
    if total_count >= MAX_TOTAL_TOKENS_PER_GUILD {
        return Err(ApiError::BadRequest(format!(
            "API トークンは失効済みを含めて 1 サーバー {MAX_TOTAL_TOKENS_PER_GUILD} 件までです。不要なトークンを削除してください"
        )));
    }

    let raw_token = generate_token();
    let prefix = display_prefix(&raw_token);
    let hash = hash_hex(raw_token.as_bytes());
    let row = sqlx::query(
        "INSERT INTO guild_api_tokens (guild_id, name, token_prefix, token_hash, scope, expires_at, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id, created_at",
    )
    .bind(guild_id)
    .bind(&name)
    .bind(&prefix)
    .bind(&hash)
    .bind(scope.as_str())
    .bind(expires_at)
    .bind(created_by)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    let id: i64 = row.get("id");
    Ok(IssuedApiToken {
        id: id.to_string(),
        name,
        token_prefix: prefix,
        scope,
        expires_at,
        created_at: row.get("created_at"),
        token: raw_token,
    })
}

/// トークンをローテーションし、新しい平文トークンを返す。失効済みは 400
pub async fn rotate(
    pool: &PgPool,
    guild_id: &str,
    token_id: i64,
) -> Result<IssuedApiToken, ApiError> {
    let mut tx = pool.begin().await?;
    let existing = sqlx::query(
        "SELECT id, name, scope, expires_at, revoked_at, created_at
         FROM guild_api_tokens
         WHERE guild_id = $1 AND id = $2
         FOR UPDATE",
    )
    .bind(guild_id)
    .bind(token_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ApiError::NotFound("API トークンが見つかりません".into()))?;

    let revoked_at: Option<DateTime<Utc>> = existing.get("revoked_at");
    if revoked_at.is_some() {
        return Err(ApiError::BadRequest(
            "失効済みの API トークンはローテーションできません".into(),
        ));
    }

    let raw_token = generate_token();
    let prefix = display_prefix(&raw_token);
    let hash = hash_hex(raw_token.as_bytes());

    sqlx::query("UPDATE guild_api_tokens SET token_prefix = $3, token_hash = $4 WHERE guild_id = $1 AND id = $2")
        .bind(guild_id)
        .bind(token_id)
        .bind(&prefix)
        .bind(&hash)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let scope_raw: String = existing.get("scope");
    Ok(IssuedApiToken {
        id: token_id.to_string(),
        name: existing.get("name"),
        token_prefix: prefix,
        scope: ApiTokenScope::from_db(&scope_raw)?,
        expires_at: existing.get("expires_at"),
        created_at: existing.get("created_at"),
        token: raw_token,
    })
}

/// トークンを失効させる
pub async fn revoke(pool: &PgPool, guild_id: &str, token_id: i64) -> Result<(), ApiError> {
    let updated = sqlx::query(
        "UPDATE guild_api_tokens
         SET revoked_at = COALESCE(revoked_at, now())
         WHERE guild_id = $1 AND id = $2",
    )
    .bind(guild_id)
    .bind(token_id)
    .execute(pool)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::NotFound("API トークンが見つかりません".into()));
    }
    Ok(())
}

/// トークンを削除する
pub async fn remove(pool: &PgPool, guild_id: &str, token_id: i64) -> Result<(), ApiError> {
    let deleted = sqlx::query("DELETE FROM guild_api_tokens WHERE guild_id = $1 AND id = $2")
        .bind(guild_id)
        .bind(token_id)
        .execute(pool)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(ApiError::NotFound("API トークンが見つかりません".into()));
    }
    Ok(())
}

/// Bearer トークンを照合する (失効・期限切れ・別ギルド・Bot 退出済みはいずれも None)
pub async fn authenticate(
    pool: &PgPool,
    guild_id: &str,
    raw_token: &str,
) -> Result<Option<AuthenticatedToken>, ApiError> {
    if !is_valid_token_format(raw_token) {
        return Ok(None);
    }
    let token_hash = hash_hex(raw_token.as_bytes());
    let row = sqlx::query(
        "SELECT t.id, t.guild_id, t.name, t.scope, t.created_by
         FROM guild_api_tokens t
         JOIN guilds g ON g.guild_id = t.guild_id
         WHERE t.token_hash = $1
           AND t.guild_id = $2
           AND t.revoked_at IS NULL
           AND (t.expires_at IS NULL OR t.expires_at > now())",
    )
    .bind(&token_hash)
    .bind(guild_id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let scope_raw: String = row.get("scope");
    Ok(Some(AuthenticatedToken {
        id: row.get("id"),
        guild_id: row.get("guild_id"),
        name: row.get("name"),
        scope: ApiTokenScope::from_db(&scope_raw)?,
        created_by: row.get("created_by"),
    }))
}

/// 呼び出しログを記録し、`last_used_at` を更新する。古いログは最新 50 件のみ残す
#[allow(clippy::too_many_arguments)]
pub async fn record_call(
    pool: &PgPool,
    token_id: i64,
    guild_id: &str,
    method: &str,
    action: &str,
    event_id: Option<i32>,
    external_id: Option<&str>,
    status: u16,
    error_kind: Option<&str>,
) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE guild_api_tokens SET last_used_at = now() WHERE id = $1 AND guild_id = $2")
        .bind(token_id)
        .bind(guild_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO guild_api_token_logs (token_id, guild_id, method, action, event_id, external_id, status, error_kind)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(token_id)
    .bind(guild_id)
    .bind(method)
    .bind(action)
    .bind(event_id)
    .bind(external_id)
    .bind(i32::from(status))
    .bind(error_kind)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM guild_api_token_logs
         WHERE token_id = $1
           AND id NOT IN (
               SELECT id FROM guild_api_token_logs WHERE token_id = $1 ORDER BY id DESC LIMIT $2
           )",
    )
    .bind(token_id)
    .bind(LOG_RETENTION_LIMIT)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// 保存済みの冪等操作を検索する
pub async fn lookup_operation(
    pool: &PgPool,
    guild_id: &str,
    token_id: i64,
    key_hash: &str,
) -> Result<Option<SavedOperation>, ApiError> {
    let row = sqlx::query(
        "SELECT request_hash, status, response_body, created_at < now() - interval '24 hours' AS expired
         FROM guild_api_token_operations
         WHERE guild_id = $1 AND token_id = $2 AND key_hash = $3",
    )
    .bind(guild_id)
    .bind(token_id)
    .bind(key_hash)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| SavedOperation {
        request_hash: r.get("request_hash"),
        status: r.get("status"),
        response_body: r.get("response_body"),
        expired: r.get("expired"),
    }))
}

/// 冪等操作の結果を保存する (同一トランザクション内でも独立接続でも呼べる)
#[allow(clippy::too_many_arguments)]
pub async fn save_operation<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    token_id: i64,
    key_hash: &str,
    request_hash: &str,
    status: u16,
    event_id: Option<i32>,
    response_body: Option<&Value>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO guild_api_token_operations (guild_id, token_id, key_hash, request_hash, status, event_id, response_body)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (guild_id, token_id, key_hash) DO NOTHING",
    )
    .bind(guild_id)
    .bind(token_id)
    .bind(key_hash)
    .bind(request_hash)
    .bind(i32::from(status))
    .bind(event_id)
    .bind(response_body)
    .execute(executor)
    .await?;
    Ok(())
}

/// 24 時間を過ぎた冪等レスポンス本文を定期削除する (墓標行は残す)
pub async fn purge_expired_operations(pool: &PgPool) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "UPDATE guild_api_token_operations
         SET response_body = NULL
         WHERE response_body IS NOT NULL
           AND created_at < now() - make_interval(hours => $1)",
    )
    .bind(IDEMPOTENCY_RETENTION_HOURS as i32)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// バックグラウンドで 1 時間ごとに期限切れ冪等レスポンスを掃除する
pub async fn run_retention(pool: PgPool) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
    loop {
        interval.tick().await;
        if let Err(err) = purge_expired_operations(&pool).await {
            tracing::error!(error = %err, "api token operation retention failed");
        }
    }
}

/// 外部 ID に紐づく有効な (ゴミ箱に入っていない) 予定 ID を引く
pub async fn find_event_id_by_external_id(
    conn: &mut PgConnection,
    guild_id: &str,
    external_id: &str,
) -> sqlx::Result<Option<i32>> {
    sqlx::query_scalar(
        "SELECT x.event_id
         FROM event_external_ids x
         JOIN events e ON e.id = x.event_id AND e.guild_id = x.guild_id
         WHERE x.guild_id = $1 AND x.external_id = $2 AND e.deleted_at IS NULL",
    )
    .bind(guild_id)
    .bind(external_id)
    .fetch_optional(conn)
    .await
}

/// 予定 ID に紐づく外部 ID を引く
pub async fn find_external_id_by_event_id(
    conn: &mut PgConnection,
    guild_id: &str,
    event_id: i32,
) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar(
        "SELECT external_id FROM event_external_ids WHERE guild_id = $1 AND event_id = $2",
    )
    .bind(guild_id)
    .bind(event_id)
    .fetch_optional(conn)
    .await
}

/// 外部 ID を予定に紐づける。既に別の予定で使われていれば Conflict を返す
pub async fn link_external_id(
    conn: &mut PgConnection,
    guild_id: &str,
    external_id: &str,
    event_id: i32,
) -> Result<(), ApiError> {
    // 同じ external_id がゴミ箱中の予定に残っていた場合も上書きせず、削除時に解除しておく設計。
    // 念のため一意制約違反は 409 に変換する
    let inserted = sqlx::query(
        "INSERT INTO event_external_ids (guild_id, external_id, event_id)
         VALUES ($1, $2, $3)
         ON CONFLICT (guild_id, external_id) DO NOTHING",
    )
    .bind(guild_id)
    .bind(external_id)
    .bind(event_id)
    .execute(conn)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(ApiError::Conflict(
            "external_id is already assigned to another event".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_generation_and_format_check() {
        let token = generate_token();
        assert!(token.starts_with(TOKEN_PREFIX));
        assert_eq!(token.len(), 68);
        assert!(is_valid_token_format(&token));
        assert_eq!(display_prefix(&token).len(), DISPLAY_PREFIX_LEN);
        assert!(!is_valid_token_format("invalid"));
        assert!(!is_valid_token_format(&format!("dct_{}", "Z".repeat(64))));
        assert_ne!(hash_hex(token.as_bytes()), token);
        assert_eq!(hash_hex(token.as_bytes()).len(), 64);
    }

    #[test]
    fn external_id_and_idempotency_key_validation() {
        assert!(validate_external_id("yt:abc-123_45.6").is_ok());
        for bad in ["", "../secret", "a/b", "a b", "..", "."] {
            assert!(validate_external_id(bad).is_err(), "{bad}");
        }
        assert!(validate_external_id(&"a".repeat(129)).is_err());

        assert!(validate_and_hash_idempotency_key("req-001").is_ok());
        assert!(validate_and_hash_idempotency_key("").is_err());
        assert!(validate_and_hash_idempotency_key("has space").is_err());
        assert!(validate_and_hash_idempotency_key(&"x".repeat(129)).is_err());
    }

    #[test]
    fn name_and_expiry_validation() {
        assert_eq!(validate_name("  CI Bot  ").unwrap(), "CI Bot");
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"a".repeat(65)).is_err());
        assert!(validate_name("line1\nline2").is_err());

        assert!(compute_expires_at(None).unwrap().is_none());
        assert!(compute_expires_at(Some(30)).unwrap().is_some());
        assert!(compute_expires_at(Some(0)).is_err());
        assert!(compute_expires_at(Some(3651)).is_err());
    }
}
