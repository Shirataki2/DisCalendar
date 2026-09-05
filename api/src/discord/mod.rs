//! Bot トークンで Discord API を呼ぶクライアント。
//!
//! ギルドへの所属・権限と、予定の作成者・更新者の表示情報を取得する。
//! 旧実装は毎リクエストで 4 回 Discord API を呼んでいたが、ここではギルド情報と
//! メンバー情報を短時間キャッシュして 0〜2 回に抑える。

pub mod permissions;
pub mod scheduled_events;

use std::{
    collections::HashMap,
    hash::{Hash as _, Hasher as _},
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
/// 利用者の操作でキャッシュを捨てられる間隔 ([`DiscordClient::refresh_permissions`]、#122)。
/// 連打されても、ギルドごとの Discord への問い合わせがこの間隔より細かくならないようにする
const REFRESH_THROTTLE: Duration = Duration::from_secs(10);
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
    /// URL に埋め込む ID が Snowflake ではない。呼び出し元が検証済みの値だけを渡すので
    /// 通常は起きないが、URL の組み立て時に必ず確認する (パスの意味が変わるのを防ぐ多層防御)
    #[error("invalid discord id")]
    InvalidId,
    /// 操作しようとしたギルドが Discord から見えない (Bot が退出・追放された、#122)。
    /// 権限のキャッシュが残っている間は、UI から操作できてしまうのでここに来る
    #[error("the bot is not in this guild")]
    GuildGone,
}

/// Discord の Snowflake ID (数字のみ、20 桁以下) か。リクエストの入り口で形式を確かめるのに使う
/// (URL に埋め込む値は、さらに [`checked_id`] で組み立て直す)
pub fn is_snowflake(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit())
}

/// URL のパスに埋め込む ID を、**数値として解釈し直した文字列**にする。
///
/// 受け取った文字列をそのまま URL に載せず、`u64` を経由して組み立て直すので、
/// `/` や `..`、クエリの区切りといったパスの意味を変える文字が入り込む余地がない
/// (呼び出し元も検証済みの値しか渡さないが、URL を作る境界でも断ち切る多層防御)。
/// 先頭ゼロなどで元の文字列と一致しない値は、別の ID を指すことになるので拒否する
fn checked_id(id: &str) -> Result<String, DiscordError> {
    if let Ok(parsed) = id.parse::<u64>() {
        let normalized = parsed.to_string();
        if normalized == id {
            return Ok(normalized);
        }
    }
    tracing::warn!("refused to build a discord url with a non-snowflake id");
    Err(DiscordError::InvalidId)
}

/// 権限の取り直し ([`DiscordClient::refresh_permissions`]) をギルドごとに直列化するロック (#122)。
///
/// ギルド ID のハッシュで固定本数から引く (ストライプ方式)。別のギルドが同じロックに当たることが
/// あるが、待ちが増えるだけで結果は変わらない。固定本数なのでギルドが増えてもメモリは増えない
struct RefreshLocks {
    stripes: [tokio::sync::Mutex<()>; Self::STRIPES],
}

impl Default for RefreshLocks {
    fn default() -> Self {
        Self {
            stripes: std::array::from_fn(|_| tokio::sync::Mutex::new(())),
        }
    }
}

impl RefreshLocks {
    const STRIPES: usize = 32;

    async fn lock(&self, guild_id: &str) -> tokio::sync::MutexGuard<'_, ()> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        guild_id.hash(&mut hasher);
        self.stripes[hasher.finish() as usize % Self::STRIPES]
            .lock()
            .await
    }
}

/// スロットルの印を立て、**この呼び出しが実際に捨てる担当か**を返す (#122)。
///
/// 判定と登録を別々に行うと、並行して来たリクエストが全部「まだ印が無い」と見て通り抜け、
/// そろって Discord を呼んでしまう。moka の entry API は同じキーの初期化を 1 つに絞るので、
/// 印が新しく入った (`is_fresh`) 呼び出しだけが `true` になる
async fn claim_refresh<K>(cache: &Cache<K, ()>, key: K) -> bool
where
    K: std::hash::Hash + Eq + Send + Sync + Clone + 'static,
{
    cache.entry(key).or_insert(()).await.is_fresh()
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
    /// (guild_id, user_id) → メンバー情報。非メンバーなら None (負のキャッシュ)
    members: Cache<(String, String), Option<Arc<ApiMember>>>,
    /// Bot の参加ギルド一覧 (管理コンソールの差分検出用)。全ギルドを何ページも取る重い呼び出しなので短時間だけ持つ
    bot_guilds: Cache<(), Arc<Vec<BotGuild>>>,
    /// 直近に [`DiscordClient::refresh_permissions`] でギルド共通の情報を捨てたギルド (#122)。
    /// 値は使わず、[`REFRESH_THROTTLE`] の間だけ「捨てた印」として置いておく
    refreshed_guilds: Cache<String, ()>,
    /// 同じく、直近にメンバー情報を捨てた `(guild_id, user_id)` (#122)。
    /// 押した人のロールはギルド共通の情報と別に絞る ([`DiscordClient::refresh_permissions`])
    refreshed_members: Cache<(String, String), ()>,
    /// 権限の取り直しをギルドごとに直列化するロック (#122)。
    /// クローンしても同じロックを共有する必要があるので [`Arc`] で持つ
    refresh_locks: Arc<RefreshLocks>,
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

#[derive(Debug, Deserialize)]
struct ApiMember {
    roles: Vec<String>,
    nick: Option<String>,
    avatar: Option<String>,
    user: Option<MemberUser>,
}

#[derive(Debug, Deserialize)]
struct MemberUser {
    username: String,
    global_name: Option<String>,
    avatar: Option<String>,
}

/// 予定の作成者・更新者の表示情報。非メンバーの場合は ID のみ。
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct MemberProfile {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl ApiMember {
    fn profile(&self, guild_id: &str, user_id: &str) -> MemberProfile {
        let display_name = self.nick.clone().or_else(|| {
            self.user
                .as_ref()
                .map(|u| u.global_name.as_ref().unwrap_or(&u.username).clone())
        });
        let avatar_url = self.avatar.as_ref().map(|hash| {
            format!("https://cdn.discordapp.com/guilds/{guild_id}/users/{user_id}/avatars/{hash}.png")
        }).or_else(|| self.user.as_ref().and_then(|u| u.avatar.as_ref()).map(|hash| {
            format!("https://cdn.discordapp.com/avatars/{user_id}/{hash}.png")
        }));
        MemberProfile {
            user_id: user_id.to_owned(),
            display_name,
            avatar_url,
        }
    }
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
            refreshed_guilds: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(REFRESH_THROTTLE)
                .build(),
            refreshed_members: Cache::builder()
                .max_capacity(100_000)
                .time_to_live(REFRESH_THROTTLE)
                .build(),
            refresh_locks: Arc::new(RefreshLocks::default()),
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

    /// Discord 側で権限を直した直後に反映させるため、権限キャッシュを取り直す (#122)。
    ///
    /// 取り直すのはギルド情報 (ロールごとの権限) と、`user_id` および Bot 自身のメンバー情報。
    /// Bot を招待し直すと変わるのは Bot の管理ロールの**権限ビット**なので、効くのは主にギルド情報の方。
    /// 戻り値は取り直した `(呼び出し元の権限, Bot が「イベントの作成」を持つか)` で、
    /// 呼び出し元がメンバーでなくなっていたら `None`。
    ///
    /// 利用者が押せる操作なので、Discord への問い合わせが増えないよう 3 段構えで抑える。
    ///
    /// 1. **キャッシュを空にせず上書きする**。捨ててから取り直すと、空いている間に来た
    ///    リクエスト (このハンドラの外側で走る [`crate::routes::GuildMember`] の抽出も含む) が
    ///    そろって miss を見て、Discord へ重複して問い合わせてしまう
    /// 2. **ギルドごとのロックで直列化する**。同じギルドに並行して来ても、待っている間に
    ///    先の呼び出しが取り直し終わるので、後続は問い合わせずに済む
    /// 3. [`REFRESH_THROTTLE`] の間は 1 回だけ実際に取り直す。見送った場合も、キャッシュの中身は
    ///    数秒以内に取り直したものになっている
    ///
    /// 絞る単位は 2 つに分けてある。ギルド共通の情報 (ロールごとの権限と Bot のロール) はギルド単位、
    /// 呼び出し元自身のロールは `(guild_id, user_id)` 単位。ギルド単位だけで絞ると、
    /// 直前に別の人が押したときにこの人のロールが古いまま (最大 [`MEMBER_TTL`]) になる
    pub async fn refresh_permissions(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> Result<Option<(MemberAccess, bool)>, DiscordError> {
        let _lock = self.refresh_locks.lock(guild_id).await;
        let member_key = (guild_id.to_owned(), user_id.to_owned());
        // 取り直しに失敗したら印を取り消す。残すと、エラーを見てすぐ押し直した人が取り直しを
        // 飛ばされ、古い値のまま「まだ変わっていません」と言われてしまう
        if claim_refresh(&self.refreshed_members, member_key.clone()).await
            && let Err(err) = self.recache_member(member_key.clone()).await
        {
            self.refreshed_members.invalidate(&member_key).await;
            return Err(err);
        }
        if claim_refresh(&self.refreshed_guilds, guild_id.to_owned()).await
            && let Err(err) = self.recache_guild_permissions(guild_id).await
        {
            self.refreshed_guilds.invalidate(guild_id).await;
            return Err(err);
        }
        // ここから先は取り直したキャッシュを読むだけ (Discord は呼ばない)
        let Some(access) = self.member_access(guild_id, user_id).await? else {
            return Ok(None);
        };
        Ok(Some((access, self.bot_create_events(guild_id).await?)))
    }

    /// メンバーの所持ロールを取り直してキャッシュを差し替える (#122)
    async fn recache_member(&self, key: (String, String)) -> Result<(), DiscordError> {
        let roles = self.fetch_member(&key.0, &key.1).await?;
        self.members.insert(key, roles).await;
        Ok(())
    }

    /// ギルド情報と Bot 自身のメンバー情報を取り直してキャッシュを差し替える (#122)
    async fn recache_guild_permissions(&self, guild_id: &str) -> Result<(), DiscordError> {
        let guild = self.fetch_guild(guild_id).await?;
        // Bot 自身のロールも取り直す (招待し直しで Bot に別のロールが付くことがある)
        let bot_user_id = self.bot_user_id().await?;
        let bot_roles = self.fetch_member(guild_id, &bot_user_id).await?;
        self.guilds.insert(guild_id.to_owned(), guild).await;
        self.members
            .insert((guild_id.to_owned(), bot_user_id), bot_roles)
            .await;
        Ok(())
    }

    /// このギルドの権限キャッシュ (ギルド情報と Bot 自身のメンバー情報) を捨てる (#122)。
    ///
    /// Discord から 403 を返されたとき (キャッシュ上の「権限あり」が誤りだと分かったとき) に呼ぶ。
    /// 利用者の操作で取り直す [`Self::refresh_permissions`] と違い、ここは書き込みの失敗時なので
    /// その場で取りに行かず、次に必要になったときに取り直させる
    pub async fn invalidate_guild_permissions(&self, guild_id: &str) {
        self.guilds.invalidate(guild_id).await;
        match self.bot_user_id().await {
            Ok(bot_user_id) => {
                self.members
                    .invalidate(&(guild_id.to_owned(), bot_user_id))
                    .await;
            }
            // Bot の ID が引けないときは諦める (TTL 切れで直る)。ギルド情報は既に捨ててある
            Err(err) => {
                tracing::warn!(guild_id, error = %err, "could not invalidate the bot's member cache");
            }
        }
    }

    /// 書き込みの結果が 403 (権限不足) なら、このギルドの権限キャッシュを捨てる (#122)。
    ///
    /// キャッシュが古くて「Bot に権限あり」と表示していたことが Discord の応答で分かった場合に、
    /// 次の取得で正しい表示 (チェックボックスの無効化と案内) に戻すため
    async fn invalidate_on_forbidden<T>(&self, guild_id: &str, result: &Result<T, DiscordError>) {
        if matches!(
            result,
            Err(DiscordError::Status { status, .. }) if *status == StatusCode::FORBIDDEN
        ) {
            self.invalidate_guild_permissions(guild_id).await;
        }
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
        let guild = self.fetch_guild(guild_id).await?;
        self.guilds.insert(guild_id.to_owned(), guild.clone()).await;
        Ok(guild)
    }

    /// ギルド情報を Discord から取る (キャッシュを見ない)。未参加なら `Ok(None)`
    async fn fetch_guild(
        &self,
        guild_id: &str,
    ) -> Result<Option<Arc<GuildSnapshot>>, DiscordError> {
        Ok(self
            .get_json::<ApiGuild>(&format!("/guilds/{}", checked_id(guild_id)?))
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
            }))
    }

    /// メンバー情報を Discord から取る (キャッシュを見ない)。非メンバーなら `Ok(None)`
    async fn fetch_member(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> Result<Option<Arc<ApiMember>>, DiscordError> {
        Ok(self
            .get_json_with_access::<ApiMember>(
                &format!(
                    "/guilds/{}/members/{}",
                    checked_id(guild_id)?,
                    checked_id(user_id)?
                ),
                false,
            )
            .await?
            .map(Arc::new))
    }

    async fn member(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> Result<Option<Arc<ApiMember>>, DiscordError> {
        let key = (checked_id(guild_id)?, checked_id(user_id)?);
        // 同じ人のプロフィール表示と認可が重なっても外部呼び出しをまとめる。
        self.members
            .try_get_with(key, self.fetch_member(guild_id, user_id))
            .await
            .map_err(|error| match Arc::try_unwrap(error) {
                Ok(error) => error,
                Err(error) => match error.as_ref() {
                    DiscordError::RateLimited => DiscordError::RateLimited,
                    _ => DiscordError::Unexpected("failed to fetch member"),
                },
            })
    }

    pub async fn member_profile(
        &self,
        guild_id: &str,
        user_id: &str,
    ) -> Result<MemberProfile, DiscordError> {
        Ok(match self.member(guild_id, user_id).await? {
            Some(member) => {
                let profile = member.profile(guild_id, user_id);
                if profile.display_name.is_none() {
                    return Err(DiscordError::Unexpected("member has no display name"));
                }
                profile
            }
            None => MemberProfile {
                user_id: user_id.to_owned(),
                display_name: None,
                avatar_url: None,
            },
        })
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

        let Some(member) = self.member(guild_id, user_id).await? else {
            return Ok(None);
        };

        let permissions = compute_base_permissions(
            &guild.id,
            &guild.owner_id,
            user_id,
            &guild.role_permissions,
            &member.roles,
        );
        Ok(Some(MemberAccess {
            guild,
            user_id: user_id.to_owned(),
            roles: member.roles.clone(),
            permissions,
        }))
    }

    /// GET して JSON にデコードする。
    /// 404 (Unknown Guild / Unknown Member) と 403 (Missing Access = Bot 未参加) は `Ok(None)`。
    /// 429 は Retry-After が短ければ 1 回だけ待って再試行する
    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>, DiscordError> {
        self.get_json_with_access(path, true).await
    }

    async fn get_json_with_access<T: DeserializeOwned>(
        &self,
        path: &str,
        missing_access_is_none: bool,
    ) -> Result<Option<T>, DiscordError> {
        let url = format!("{}{path}", self.api_base);
        let mut retried = false;
        loop {
            let res = self.http.get(&url).send().await?;
            let status = res.status();
            if status.is_success() {
                return Ok(Some(res.json().await?));
            }
            if status == StatusCode::NOT_FOUND
                || (missing_access_is_none && status == StatusCode::FORBIDDEN)
            {
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

#[cfg(test)]
mod tests {
    use super::{checked_id, is_snowflake};

    #[test]
    fn member_profile_prefers_guild_name_and_avatar() {
        let mut member: super::ApiMember = serde_json::from_value(serde_json::json!({
            "roles": [], "nick": "サーバー名", "avatar": "local",
            "user": {"username": "username", "global_name": "表示名", "avatar": "global"}
        }))
        .unwrap();
        let profile = member.profile("111", "333");
        assert_eq!(profile.display_name.as_deref(), Some("サーバー名"));
        assert_eq!(
            profile.avatar_url.as_deref(),
            Some("https://cdn.discordapp.com/guilds/111/users/333/avatars/local.png")
        );
        member.nick = None;
        member.avatar = None;
        let profile = member.profile("111", "333");
        assert_eq!(profile.display_name.as_deref(), Some("表示名"));
        assert_eq!(
            profile.avatar_url.as_deref(),
            Some("https://cdn.discordapp.com/avatars/333/global.png")
        );
        member.user.as_mut().unwrap().global_name = None;
        member.user.as_mut().unwrap().avatar = None;
        let profile = member.profile("111", "333");
        assert_eq!(profile.display_name.as_deref(), Some("username"));
        assert_eq!(profile.avatar_url, None);
    }

    #[tokio::test]
    async fn member_profiles_share_cache_with_permissions_and_cache_departures() {
        use actix_web::{App, HttpResponse, HttpServer, web};
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let count = Arc::new(AtomicUsize::new(0));
        let requests = count.clone();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = HttpServer::new(move || {
            let count = requests.clone();
            App::new().route("/guilds/111", web::get().to(|| async {
                HttpResponse::Ok().json(serde_json::json!({"id":"111","name":"guild","owner_id":"333","roles":[]}))
            })).route("/guilds/111/members/{id}", web::get().to(move |id: web::Path<String>| {
                let count = count.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    if id.as_str() == "333" {
                        return HttpResponse::Ok().json(serde_json::json!({"roles":[],"user":{"username":"member"}}));
                    }
                    if id.as_str() == "555" { return HttpResponse::Forbidden().finish(); }
                    HttpResponse::NotFound().finish()
                }
            }))
        }).listen(listener).unwrap().run();
        let handle = server.handle();
        tokio::spawn(server);
        let client = super::DiscordClient::new("test", &format!("http://{address}")).unwrap();
        assert!(client.member_access("111", "333").await.unwrap().is_some());
        assert_eq!(
            client
                .member_profile("111", "333")
                .await
                .unwrap()
                .display_name
                .as_deref(),
            Some("member")
        );
        for _ in 0..2 {
            assert_eq!(
                client
                    .member_profile("111", "444")
                    .await
                    .unwrap()
                    .display_name,
                None
            );
        }
        assert_eq!(count.load(Ordering::SeqCst), 2);
        assert!(client.member_profile("111", "555").await.is_err());
        handle.stop(true).await;
    }

    #[test]
    fn checked_id_accepts_snowflakes() {
        assert_eq!(
            checked_id("782502586817314816").unwrap(),
            "782502586817314816"
        );
        assert_eq!(checked_id("0").unwrap(), "0");
    }

    #[test]
    fn checked_id_rejects_values_that_would_change_the_url() {
        // パスの意味を変えうる文字は、URL に載る前に弾く
        for id in [
            "",
            "abc",
            "1/2",
            "..",
            "../../users/@me",
            "1?query=x",
            "1#frag",
            "1%2F2",
            "1 2",
            " 1",
            "+1",
            "-1",
            // u64 に収まらない (Discord の Snowflake は 64bit)
            "99999999999999999999",
        ] {
            assert!(checked_id(id).is_err(), "{id}");
        }
    }

    #[test]
    fn checked_id_rejects_ids_that_do_not_round_trip() {
        // 先頭ゼロを落とすと別の ID を指してしまうので、数値としては読めても拒否する
        assert!(checked_id("0123456789012345678").is_err());
        assert!(!is_snowflake(""));
    }
}
