//! Bot トークンで Discord API を呼ぶクライアント。
//!
//! ユーザーがギルドのメンバーかどうかと、ギルド内での権限を調べるためだけに使う。
//! 旧実装は毎リクエストで 4 回 Discord API を呼んでいたが、ここではギルド情報と
//! メンバー情報を短時間キャッシュして 0〜2 回に抑える。

pub mod permissions;
pub mod scheduled_events;

use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
    time::Duration,
};

use anyhow::Context as _;
use moka::future::Cache;
use reqwest::{StatusCode, header};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub use self::permissions::Permissions;
use self::permissions::compute_base_permissions;

/// Discord API の既定のベース URL。E2E テストではモックに差し替える ([`DiscordClient::new`])
pub const DEFAULT_API_BASE: &str = "https://discord.com/api/v10";
const USER_AGENT: &str = concat!(
    "DiscordBot (https://discalendar.app, ",
    env!("CARGO_PKG_VERSION"),
    ")"
);
/// ギルド (ロール一覧・オーナー) のキャッシュ期間
const GUILD_TTL: Duration = Duration::from_secs(300);
/// メンバー (所持ロール) のキャッシュ期間。ロール変更や退出の反映はこの時間だけ遅れる
const MEMBER_TTL: Duration = Duration::from_secs(60);
/// Bot の参加ギルド一覧 ([`DiscordClient::bot_guilds`]) のキャッシュ期間
const BOT_GUILDS_TTL: Duration = Duration::from_secs(60);
/// `GET /users/@me/guilds` の 1 ページの件数 (Discord の上限は 200)
const BOT_GUILDS_PAGE_SIZE: usize = 200;
/// 辿るページ数の上限 (200 件 × 100 ページ = 20,000 ギルド)。無限ループにしないための安全網
const BOT_GUILDS_MAX_PAGES: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum DiscordError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Discord API returned {status}: {body}")]
    Status { status: StatusCode, body: String },
    #[error("rate limited by Discord API")]
    RateLimited,
    #[error("unexpected response from Discord API: {0}")]
    Unexpected(&'static str),
}

/// 権限計算に必要な最小限のギルド情報
#[derive(Debug, Clone)]
pub struct GuildSnapshot {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub owner_id: String,
    /// role_id → permissions ビット。@everyone は role_id == guild_id
    pub role_permissions: HashMap<String, u64>,
}

impl GuildSnapshot {
    pub fn icon_url(&self) -> Option<String> {
        self.icon
            .as_ref()
            .map(|icon| format!("https://cdn.discordapp.com/icons/{}/{}.png", self.id, icon))
    }
}

/// Bot が参加しているギルドの一覧 (`GET /users/@me/guilds`) の 1 件。
/// ロールなどは返らないので権限計算には使えない (管理コンソールの差分検出用、#37)
#[derive(Debug, Clone, Serialize)]
pub struct BotGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
}

/// あるユーザーのギルドへのアクセス情報 (メンバーであることが確認済み)
#[derive(Debug, Clone)]
pub struct MemberAccess {
    pub guild: Arc<GuildSnapshot>,
    pub user_id: String,
    pub roles: Vec<String>,
    pub permissions: Permissions,
}

#[derive(Clone)]
pub struct DiscordClient {
    http: reqwest::Client,
    /// API のベース URL (末尾の `/` なし)。通常は [`DEFAULT_API_BASE`]
    api_base: String,
    /// guild_id → ギルド情報。Bot が未参加なら None (負のキャッシュ)
    guilds: Cache<String, Option<Arc<GuildSnapshot>>>,
    /// (guild_id, user_id) → メンバーの所持ロール。非メンバーなら None (負のキャッシュ)
    members: Cache<(String, String), Option<Arc<Vec<String>>>>,
    /// Bot の参加ギルド一覧 (管理コンソールの差分検出用)。全ギルドを何ページも取る重い呼び出しなので短時間だけ持つ
    bot_guilds: Cache<(), Arc<Vec<BotGuild>>>,
    /// Bot 自身のユーザー ID (`GET /users/@me`)。トークンが変わらない限り不変なのでプロセス中ずっと持つ
    bot_user_id: Arc<OnceLock<String>>,
}

#[derive(Deserialize)]
struct ApiGuild {
    id: String,
    name: String,
    icon: Option<String>,
    owner_id: String,
    roles: Vec<ApiRole>,
}

#[derive(Deserialize)]
struct ApiRole {
    id: String,
    /// Discord は permissions を文字列化した整数で返す
    permissions: String,
}

#[derive(Deserialize)]
struct ApiMember {
    roles: Vec<String>,
}

/// `GET /users/@me` が返すユーザー情報のうち使う部分
#[derive(Deserialize)]
struct ApiUser {
    id: String,
}

/// `GET /users/@me/guilds` が返す部分的なギルド情報
#[derive(Deserialize)]
struct ApiPartialGuild {
    id: String,
    name: String,
    icon: Option<String>,
}

impl DiscordClient {
    /// `api_base` は Discord API のベース URL (`DISCORD_API_BASE_URL`)。
    /// 本番では [`DEFAULT_API_BASE`] で、E2E テスト (web/e2e) ではモックサーバーの URL を渡す
    pub fn new(bot_token: &str, api_base: &str) -> anyhow::Result<Self> {
        let mut auth = header::HeaderValue::from_str(&format!("Bot {bot_token}"))
            .context("DISCORD_BOT_TOKEN contains invalid characters")?;
        auth.set_sensitive(true);
        let mut headers = header::HeaderMap::new();
        headers.insert(header::AUTHORIZATION, auth);

        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .default_headers(headers)
            .timeout(Duration::from_secs(10))
            .build()
            .context("failed to build Discord HTTP client")?;

        Ok(Self {
            http,
            api_base: api_base.trim_end_matches('/').to_owned(),
            guilds: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(GUILD_TTL)
                .build(),
            members: Cache::builder()
                .max_capacity(100_000)
                .time_to_live(MEMBER_TTL)
                .build(),
            bot_guilds: Cache::builder()
                .max_capacity(1)
                .time_to_live(BOT_GUILDS_TTL)
                .build(),
            bot_user_id: Arc::new(OnceLock::new()),
        })
    }

    /// Bot 自身のユーザー ID (`GET /users/@me`)。初回だけ Discord に問い合わせ、以後はキャッシュを返す
    async fn bot_user_id(&self) -> Result<String, DiscordError> {
        if let Some(id) = self.bot_user_id.get() {
            return Ok(id.clone());
        }
        let user: ApiUser = self
            .get_json("/users/@me")
            .await?
            // Bot トークンがあれば 403 / 404 にはならない
            .ok_or(DiscordError::Unexpected("cannot fetch the bot user"))?;
        // 競合してもトークンが同じなら同じ ID なので、先に入った値をそのまま使う
        Ok(self.bot_user_id.get_or_init(|| user.id).clone())
    }

    /// Bot 自身がギルドで「イベントの作成」権限を持つか (#94)。
    /// Bot が未参加なら `false`。ギルド・メンバーのキャッシュに乗るため、
    /// 再招待などの権限変更が反映されるまで最大で数分の遅れがある
    pub async fn bot_create_events(&self, guild_id: &str) -> Result<bool, DiscordError> {
        let bot_user_id = self.bot_user_id().await?;
        Ok(self
            .member_access(guild_id, &bot_user_id)
            .await?
            .is_some_and(|access| access.permissions.create_events()))
    }

    /// Bot が参加している全ギルド (`GET /users/@me/guilds` を 200 件ずつ辿る)。
    /// 管理コンソールの差分検出 (#37) 専用。何度も押されても Discord に負荷をかけないよう
    /// [`BOT_GUILDS_TTL`] の間はキャッシュを返す。
    ///
    /// カーソルを付けずに呼ぶと Discord は ID の**降順**で返し、`after` を付けると**昇順**になる。
    /// 1 ページ目だけカーソル無しにすると 2 ページ目から向きが変わり、重複と取りこぼしが出る
    /// (200 ギルドを超えたときだけ表面化する) ので、1 ページ目から `after=0` で昇順に統一する
    pub async fn bot_guilds(&self) -> Result<Arc<Vec<BotGuild>>, DiscordError> {
        if let Some(cached) = self.bot_guilds.get(&()).await {
            return Ok(cached);
        }
        let mut all: Vec<BotGuild> = Vec::new();
        // Snowflake の 0 より大きい ID から昇順に (どのギルドの ID も 0 より大きい)
        let mut after = "0".to_owned();
        for _ in 0..BOT_GUILDS_MAX_PAGES {
            let path = format!("/users/@me/guilds?limit={BOT_GUILDS_PAGE_SIZE}&after={after}");
            let page: Vec<ApiPartialGuild> = self
                .get_json(&path)
                .await?
                // このエンドポイントは Bot トークンがあれば 403 / 404 にはならない
                .ok_or(DiscordError::Unexpected("cannot list the bot's guilds"))?;
            let last = page.last().map(|g| g.id.clone());
            let count = page.len();
            all.extend(page.into_iter().map(|g| BotGuild {
                id: g.id,
                name: g.name,
                icon: g.icon,
            }));
            // 昇順なので、次のページはこのページの末尾 (最大の ID) の続きから
            match last {
                Some(id) if count >= BOT_GUILDS_PAGE_SIZE => after = id,
                // 上限に満たなければ最後のページ (空のページも含む)
                _ => {
                    let all = Arc::new(all);
                    self.bot_guilds.insert((), all.clone()).await;
                    return Ok(all);
                }
            }
        }
        // ここまで来るのは 20,000 ギルドを超えたとき (現実には起きない)。
        // 途中までの一覧を「全部」として差分を出すと誤検出になるのでエラーにする
        Err(DiscordError::Unexpected(
            "the bot is in too many guilds to list",
        ))
    }

    /// Bot が参加しているギルドの情報。未参加 (404 / 403) なら `Ok(None)`
    pub async fn guild(&self, guild_id: &str) -> Result<Option<Arc<GuildSnapshot>>, DiscordError> {
        if let Some(cached) = self.guilds.get(guild_id).await {
            return Ok(cached);
        }
        let guild = self
            .get_json::<ApiGuild>(&format!("/guilds/{guild_id}"))
            .await?
            .map(|g| {
                Arc::new(GuildSnapshot {
                    id: g.id,
                    name: g.name,
                    icon: g.icon,
                    owner_id: g.owner_id,
                    role_permissions: g
                        .roles
                        .into_iter()
                        .map(|r| (r.id, r.permissions.parse::<u64>().unwrap_or(0)))
                        .collect(),
                })
            });
        self.guilds.insert(guild_id.to_owned(), guild.clone()).await;
        Ok(guild)
    }

    /// ユーザーがギルドのメンバーなら権限付きで返す。Bot 未参加または非メンバーなら `Ok(None)`
    pub async fn member_access(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> Result<Option<MemberAccess>, DiscordError> {
        let Some(guild) = self.guild(guild_id).await? else {
            return Ok(None);
        };

        let key = (guild_id.to_owned(), user_id.to_owned());
        let roles = match self.members.get(&key).await {
            Some(cached) => cached,
            None => {
                let fetched = self
                    .get_json::<ApiMember>(&format!("/guilds/{guild_id}/members/{user_id}"))
                    .await?
                    .map(|m| Arc::new(m.roles));
                self.members.insert(key, fetched.clone()).await;
                fetched
            }
        };
        let Some(roles) = roles else {
            return Ok(None);
        };

        let permissions = compute_base_permissions(
            &guild.id,
            &guild.owner_id,
            user_id,
            &guild.role_permissions,
            &roles,
        );
        Ok(Some(MemberAccess {
            guild,
            user_id: user_id.to_owned(),
            roles: roles.to_vec(),
            permissions,
        }))
    }

    /// GET して JSON にデコードする。
    /// 404 (Unknown Guild / Unknown Member) と 403 (Missing Access = Bot 未参加) は `Ok(None)`。
    /// 429 は Retry-After が短ければ 1 回だけ待って再試行する
    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>, DiscordError> {
        let url = format!("{}{path}", self.api_base);
        let mut retried = false;
        loop {
            let res = self.http.get(&url).send().await?;
            let status = res.status();
            if status.is_success() {
                return Ok(Some(res.json().await?));
            }
            if status == StatusCode::NOT_FOUND || status == StatusCode::FORBIDDEN {
                tracing::debug!(%path, %status, "discord resource not accessible");
                return Ok(None);
            }
            if status == StatusCode::TOO_MANY_REQUESTS {
                let retry_after = res
                    .headers()
                    .get(header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(1.0);
                if !retried && retry_after <= 2.0 {
                    tracing::warn!(%path, retry_after, "discord rate limited, retrying once");
                    retried = true;
                    tokio::time::sleep(Duration::from_secs_f64(retry_after)).await;
                    continue;
                }
                tracing::warn!(%path, retry_after, "discord rate limited");
                return Err(DiscordError::RateLimited);
            }
            let body = res.text().await.unwrap_or_default();
            tracing::warn!(%path, %status, %body, "discord api error");
            return Err(DiscordError::Status { status, body });
        }
    }

    /// 書き込みリクエスト (POST / PATCH / DELETE) を送り、成功したらレスポンスボディを文字列で返す
    /// (204 なら空文字列)。404 (対象が既に無い) は `Ok(None)`。
    /// GET ([`Self::get_json`]) と違い **403 はエラーのまま返す** (書き込みの 403 は Bot の
    /// 「イベントの管理」権限の不足で、呼び出し元が利用者に案内を返すため)。
    /// 429 は Retry-After が短ければ 1 回だけ待って再試行する
    async fn send(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<Option<String>, DiscordError> {
        let url = format!("{}{path}", self.api_base);
        let mut retried = false;
        loop {
            let mut req = self.http.request(method.clone(), &url);
            if let Some(body) = body {
                req = req.json(body);
            }
            let res = req.send().await?;
            let status = res.status();
            if status.is_success() {
                return Ok(Some(res.text().await?));
            }
            if status == StatusCode::NOT_FOUND {
                tracing::debug!(%path, %status, "discord resource not found");
                return Ok(None);
            }
            if status == StatusCode::TOO_MANY_REQUESTS {
                let retry_after = res
                    .headers()
                    .get(header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(1.0);
                if !retried && retry_after <= 2.0 {
                    tracing::warn!(%path, retry_after, "discord rate limited, retrying once");
                    retried = true;
                    tokio::time::sleep(Duration::from_secs_f64(retry_after)).await;
                    continue;
                }
                tracing::warn!(%path, retry_after, "discord rate limited");
                return Err(DiscordError::RateLimited);
            }
            let body = res.text().await.unwrap_or_default();
            tracing::warn!(%path, %status, %body, "discord api error");
            return Err(DiscordError::Status { status, body });
        }
    }
}
