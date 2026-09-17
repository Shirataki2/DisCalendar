//! R2 の一時アップロードと確定オブジェクト。URLとSDKエラーには認証情報があるのでログに出さない。
use std::{collections::BTreeMap, time::Duration};

use aws_sdk_s3::{
    Client,
    config::{
        BehaviorVersion, Credentials, Region, RequestChecksumCalculation,
        ResponseChecksumValidation, retry::RetryConfig, timeout::TimeoutConfig,
    },
    presigning::PresigningConfig,
    types::MetadataDirective,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::{
    error::ApiError,
    models::attachments::{self, Attachment},
};

pub const URL_TTL: u64 = 300;

#[derive(Clone, Default)]
pub struct AttachmentStorage(pub Option<Store>);

#[derive(Clone)]
pub struct Store {
    pub client: Client,
    pub bucket: String,
}

impl AttachmentStorage {
    pub fn from_env() -> anyhow::Result<Self> {
        let keys = [
            "ATTACHMENTS_S3_ENDPOINT",
            "ATTACHMENTS_BUCKET",
            "ATTACHMENTS_ACCESS_KEY_ID",
            "ATTACHMENTS_SECRET_ACCESS_KEY",
        ];
        let values: Vec<_> = keys
            .iter()
            .map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
            .collect();
        if values.iter().all(Option::is_none) {
            return Ok(Self(None));
        }
        anyhow::ensure!(
            values.iter().all(Option::is_some),
            "ATTACHMENTS_* の4項目をすべて設定してください"
        );
        let values: Vec<_> = values.into_iter().flatten().collect();
        let endpoint = reqwest::Url::parse(&values[0])
            .map_err(|_| anyhow::anyhow!("ATTACHMENTS_S3_ENDPOINT が不正です"))?;
        anyhow::ensure!(
            endpoint.scheme() == "https"
                || (endpoint.scheme() == "http"
                    && matches!(endpoint.host_str(), Some("localhost" | "127.0.0.1"))),
            "添付の接続先にはHTTPSが必要です（ローカルテストを除く）"
        );
        Ok(Self(Some(Store::new(
            &values[0], &values[1], &values[2], &values[3],
        ))))
    }

    pub fn get(&self) -> Result<&Store, ApiError> {
        self.0
            .as_ref()
            .ok_or_else(|| ApiError::Unavailable("添付ファイルの保存先が設定されていません".into()))
    }
}

#[derive(Serialize, ToSchema)]
pub struct SignedUrl {
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub expires_at: DateTime<Utc>,
}

fn storage_error() -> ApiError {
    tracing::error!("attachment storage request failed");
    ApiError::Unavailable("ファイルの保存先に接続できません。時間をおいて再試行してください".into())
}

impl Store {
    pub fn new(endpoint: &str, bucket: &str, key: &str, secret: &str) -> Self {
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .endpoint_url(endpoint)
            .region(Region::new("auto"))
            .credentials_provider(Credentials::new(key, secret, None, None, "attachments"))
            .force_path_style(true)
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .response_checksum_validation(ResponseChecksumValidation::WhenRequired)
            .retry_config(RetryConfig::standard().with_max_attempts(1))
            .timeout_config(
                TimeoutConfig::builder()
                    .operation_timeout(Duration::from_secs(20))
                    .build(),
            )
            .build();
        Self {
            client: Client::from_conf(config),
            bucket: bucket.into(),
        }
    }

    pub async fn upload_url(&self, row: &Attachment) -> Result<SignedUrl, ApiError> {
        let request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&row.temporary_key)
            .content_type(&row.content_type)
            .content_length(row.size)
            .if_none_match("*")
            .presigned(
                PresigningConfig::expires_in(Duration::from_secs(URL_TTL))
                    .map_err(|_| storage_error())?,
            )
            .await
            .map_err(|_| storage_error())?;
        // Content-Length はブラウザ自身が File のサイズから付ける。JSから指定できないが署名には含める。
        Ok(SignedUrl {
            url: request.uri().to_owned(),
            headers: request
                .headers()
                .filter(|(name, _)| {
                    !name.eq_ignore_ascii_case("content-length")
                        && !name.eq_ignore_ascii_case("host")
                })
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .collect(),
            expires_at: Utc::now() + chrono::Duration::seconds(URL_TTL as i64),
        })
    }

    pub async fn read_url(&self, row: &Attachment, preview: bool) -> Result<SignedUrl, ApiError> {
        if preview && !row.content_type.starts_with("image/") {
            return Err(ApiError::BadRequest(
                "この形式はプレビューできません".into(),
            ));
        }
        let disposition = if preview {
            "inline".to_owned()
        } else {
            download_disposition(&row.filename)
        };
        let request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&row.object_key)
            .response_content_type(&row.content_type)
            .response_content_disposition(disposition)
            .response_cache_control("no-store")
            .presigned(
                PresigningConfig::expires_in(Duration::from_secs(URL_TTL))
                    .map_err(|_| storage_error())?,
            )
            .await
            .map_err(|_| storage_error())?;
        Ok(SignedUrl {
            url: request.uri().into(),
            headers: BTreeMap::new(),
            expires_at: Utc::now() + chrono::Duration::seconds(URL_TTL as i64),
        })
    }

    pub async fn verify_and_copy(&self, row: &Attachment) -> Result<(), ApiError> {
        let result = tokio::time::timeout(Duration::from_secs(30), async {
            let head = self
                .client
                .head_object()
                .bucket(&self.bucket)
                .key(&row.temporary_key)
                .send()
                .await
                .map_err(|_| storage_error())?;
            if head.content_length() != Some(row.size)
                || head.content_type() != Some(row.content_type.as_str())
            {
                return Err(ApiError::BadRequest(
                    "送信されたファイルのサイズ・種類が選択時と異なります".into(),
                ));
            }
            let etag = head.e_tag().ok_or_else(storage_error)?;
            // 先頭だけを取得してメモリ消費を制限する。HEAD と別バージョンになった場合は確定しない。
            let object = self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(&row.temporary_key)
                .range("bytes=0-15")
                .if_match(etag)
                .send()
                .await
                .map_err(|_| storage_error())?;
            let bytes = object
                .body
                .collect()
                .await
                .map_err(|_| storage_error())?
                .into_bytes();
            if attachments::detected_type(&bytes) != Some(row.content_type.as_str()) {
                return Err(ApiError::BadRequest(
                    "ファイルの内容がJPEG・PNG・WebP・PDFのいずれとも一致しません".into(),
                ));
            }
            self.client
                .copy_object()
                .bucket(&self.bucket)
                .key(&row.object_key)
                .copy_source(format!("{}/{}", self.bucket, row.temporary_key))
                .copy_source_if_match(etag)
                .metadata_directive(MetadataDirective::Replace)
                .content_type(&row.content_type)
                .content_disposition(download_disposition(&row.filename))
                .cache_control("no-store")
                .send()
                .await
                .map_err(|_| storage_error())?;
            Ok(())
        })
        .await;
        result.map_err(|_| storage_error())?
    }

    pub async fn delete(&self, key: &str) -> Result<(), ApiError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|_| storage_error())?;
        Ok(())
    }
}

fn download_disposition(filename: &str) -> String {
    // RFC 5987。UTF-8の各バイトをエンコードし、改行・引用符のヘッダ注入も防ぐ。
    let encoded: String = filename.bytes().map(|b| format!("%{b:02X}")).collect();
    format!("attachment; filename=\"download\"; filename*=UTF-8''{encoded}")
}

pub async fn run(pool: PgPool, store: Store) {
    loop {
        if cleanup_once(&pool, &store).await.is_err() {
            tracing::error!("attachment cleanup failed; will retry in one minute");
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

pub async fn cleanup_once(pool: &PgPool, store: &Store) -> Result<(), ApiError> {
    // 期限切れの予約もDELETEトリガーに流す。確定処理が行ロックを持つ場合は次回に回す。
    sqlx::query("DELETE FROM event_attachments WHERE id IN (SELECT id FROM event_attachments WHERE NOT ready AND expires_at <= now() FOR UPDATE SKIP LOCKED LIMIT 100)")
        .execute(pool).await?;
    for _ in 0..100 {
        let mut tx = pool.begin().await?;
        let key: Option<String> = sqlx::query_scalar("SELECT object_key FROM attachment_deletions WHERE next_attempt_at <= now() ORDER BY next_attempt_at FOR UPDATE SKIP LOCKED LIMIT 1")
            .fetch_optional(&mut *tx).await?;
        let Some(key) = key else {
            break;
        };
        if store.delete(&key).await.is_ok() {
            sqlx::query("DELETE FROM attachment_deletions WHERE object_key=$1")
                .bind(&key)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query("UPDATE attachment_deletions SET attempts=attempts+1,next_attempt_at=now()+INTERVAL '1 minute' WHERE object_key=$1")
                .bind(&key).execute(&mut *tx).await?;
        }
        tx.commit().await?;
    }
    Ok(())
}
