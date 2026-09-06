//! Discord のパーミッション計算。
//!
//! 旧実装はユーザーの OAuth トークンで `/users/@me/guilds` を呼んで permissions を得ていたが、
//! 新実装は Bot トークンで取得したギルドのロール一覧とメンバーのロールから
//! ギルドレベルの基本パーミッションを計算する。利用者の認可 (restricted モードなど) は
//! チャンネル上書きを考慮しないギルドレベルの値で判定し、Bot がチャンネルに投稿できるかの判定
//! (通知先の選択、#181) だけはチャンネルの上書きまで反映した値 ([`compute_channel_permissions`]) を使う。
//! <https://discord.com/developers/docs/topics/permissions#permission-overwrites>

use std::collections::HashMap;

/// パーミッションのビット集合。値は discord-api-types の `PermissionFlagsBits` と同じ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Permissions(u64);

impl Permissions {
    pub const ADMINISTRATOR: u64 = 1 << 3;
    pub const MANAGE_GUILD: u64 = 1 << 5;
    pub const VIEW_CHANNEL: u64 = 1 << 10;
    pub const SEND_MESSAGES: u64 = 1 << 11;
    pub const MANAGE_MESSAGES: u64 = 1 << 13;
    pub const EMBED_LINKS: u64 = 1 << 14;
    pub const MANAGE_ROLES: u64 = 1 << 28;
    pub const CREATE_EVENTS: u64 = 1 << 44;

    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    /// ADMINISTRATOR を持っていれば全権限を持つ扱い (Discord の仕様どおり)
    pub const fn has(self, bit: u64) -> bool {
        self.0 & Self::ADMINISTRATOR != 0 || self.0 & bit == bit
    }

    pub const fn administrator(self) -> bool {
        self.0 & Self::ADMINISTRATOR != 0
    }

    pub const fn manage_guild(self) -> bool {
        self.has(Self::MANAGE_GUILD)
    }

    pub const fn manage_messages(self) -> bool {
        self.has(Self::MANAGE_MESSAGES)
    }

    pub const fn manage_roles(self) -> bool {
        self.has(Self::MANAGE_ROLES)
    }

    /// Discord スケジュールイベントの作成に必要な「イベントの作成」(#94)。Bot 自身の権限判定に使う。
    /// 自分が作ったイベントの変更・削除もこの権限でできる (「イベントの管理」(1 << 33) は
    /// 他人が作ったイベントの操作用で、この用途には要らない)
    pub const fn create_events(self) -> bool {
        self.has(Self::CREATE_EVENTS)
    }

    /// 旧実装と同じ「サーバー管理」判定: 管理者 / サーバー管理 / メッセージの管理 / ロールの管理 のいずれか。
    /// restricted モードのギルドで予定を編集できるかどうか、サーバー設定を変更できるかどうかに使う
    pub const fn can_manage_server(self) -> bool {
        self.administrator() || self.manage_guild() || self.manage_messages() || self.manage_roles()
    }
}

/// ギルドレベルの基本パーミッションを計算する。
///
/// - オーナーは ADMINISTRATOR 扱い
/// - それ以外は @everyone ロール (id == guild_id) とメンバーの持つロールの OR
pub fn compute_base_permissions(
    guild_id: &str,
    owner_id: &str,
    user_id: &str,
    role_permissions: &HashMap<String, u64>,
    member_roles: &[String],
) -> Permissions {
    if owner_id == user_id {
        return Permissions(Permissions::ADMINISTRATOR);
    }
    let mut bits = role_permissions.get(guild_id).copied().unwrap_or(0);
    for role in member_roles {
        bits |= role_permissions.get(role).copied().unwrap_or(0);
    }
    Permissions(bits)
}

/// チャンネルの権限上書き (`GET /guilds/{id}/channels` の `permission_overwrites` の 1 件)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionOverwrite {
    /// ロール ID またはユーザー ID
    pub id: String,
    /// 0 = ロール、1 = メンバー
    pub kind: u8,
    pub allow: u64,
    pub deny: u64,
}

impl PermissionOverwrite {
    pub const ROLE: u8 = 0;
    pub const MEMBER: u8 = 1;
}

/// あるチャンネルでの実効パーミッション。Discord の手順どおりに
/// @everyone の上書き → メンバーが持つロールの上書き (deny をまとめて外し allow をまとめて足す)
/// → メンバー個人の上書き の順に適用する。ADMINISTRATOR を持っていれば上書きに関わらず全権限。
/// Bot (`bot/src/checks.rs` の `bot_permissions_in`、serenity の `user_permissions_in`) と同じ結果になる
pub fn compute_channel_permissions(
    base: Permissions,
    guild_id: &str,
    user_id: &str,
    member_roles: &[String],
    overwrites: &[PermissionOverwrite],
) -> Permissions {
    if base.administrator() {
        return Permissions(u64::MAX);
    }
    let mut bits = base.0;
    if let Some(everyone) = overwrites
        .iter()
        .find(|o| o.kind == PermissionOverwrite::ROLE && o.id == guild_id)
    {
        bits &= !everyone.deny;
        bits |= everyone.allow;
    }
    let (mut allow, mut deny) = (0u64, 0u64);
    for overwrite in overwrites
        .iter()
        .filter(|o| o.kind == PermissionOverwrite::ROLE && member_roles.contains(&o.id))
    {
        allow |= overwrite.allow;
        deny |= overwrite.deny;
    }
    bits &= !deny;
    bits |= allow;
    if let Some(member) = overwrites
        .iter()
        .find(|o| o.kind == PermissionOverwrite::MEMBER && o.id == user_id)
    {
        bits &= !member.deny;
        bits |= member.allow;
    }
    Permissions(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles(pairs: &[(&str, u64)]) -> HashMap<String, u64> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
    }

    fn overwrite(id: &str, kind: u8, allow: u64, deny: u64) -> PermissionOverwrite {
        PermissionOverwrite {
            id: id.to_owned(),
            kind,
            allow,
            deny,
        }
    }

    #[test]
    fn channel_overwrites_apply_in_discord_order() {
        let base = Permissions::from_bits(Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES);
        let member_roles = ["r1".to_owned()];
        // @everyone で送信を禁止し、ロールで埋め込みを許可、本人で送信を許可し直す
        let overwrites = [
            overwrite(
                "g",
                PermissionOverwrite::ROLE,
                0,
                Permissions::SEND_MESSAGES,
            ),
            overwrite("r1", PermissionOverwrite::ROLE, Permissions::EMBED_LINKS, 0),
            overwrite(
                "bot",
                PermissionOverwrite::MEMBER,
                Permissions::SEND_MESSAGES,
                0,
            ),
        ];
        let p = compute_channel_permissions(base, "g", "bot", &member_roles, &overwrites);
        assert_eq!(
            p.bits(),
            Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::EMBED_LINKS
        );
        // 本人の上書きが無ければ @everyone の禁止が残る
        let p = compute_channel_permissions(base, "g", "bot", &member_roles, &overwrites[..2]);
        assert_eq!(
            p.bits(),
            Permissions::VIEW_CHANNEL | Permissions::EMBED_LINKS
        );
    }

    #[test]
    fn role_overwrites_are_merged_before_applying() {
        // 同じ権限を一方のロールが許可し、もう一方が禁止していれば、許可が勝つ (Discord の仕様)
        let base = Permissions::from_bits(0);
        let member_roles = ["r1".to_owned(), "r2".to_owned()];
        let overwrites = [
            overwrite(
                "r1",
                PermissionOverwrite::ROLE,
                0,
                Permissions::VIEW_CHANNEL,
            ),
            overwrite(
                "r2",
                PermissionOverwrite::ROLE,
                Permissions::VIEW_CHANNEL,
                0,
            ),
        ];
        let p = compute_channel_permissions(base, "g", "u", &member_roles, &overwrites);
        assert!(p.has(Permissions::VIEW_CHANNEL));
    }

    #[test]
    fn overwrites_for_other_roles_and_members_are_ignored() {
        let base = Permissions::from_bits(Permissions::VIEW_CHANNEL);
        let overwrites = [
            overwrite(
                "other",
                PermissionOverwrite::ROLE,
                0,
                Permissions::VIEW_CHANNEL,
            ),
            overwrite(
                "someone",
                PermissionOverwrite::MEMBER,
                0,
                Permissions::VIEW_CHANNEL,
            ),
            // ロール ID とユーザー ID が同じ値でも種別が違えば別物
            overwrite("u", PermissionOverwrite::ROLE, 0, Permissions::VIEW_CHANNEL),
        ];
        let p = compute_channel_permissions(base, "g", "u", &[], &overwrites);
        assert!(p.has(Permissions::VIEW_CHANNEL));
    }

    #[test]
    fn administrators_ignore_channel_overwrites() {
        let base = Permissions::from_bits(Permissions::ADMINISTRATOR);
        let overwrites = [overwrite(
            "g",
            PermissionOverwrite::ROLE,
            0,
            Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
        )];
        let p = compute_channel_permissions(base, "g", "u", &[], &overwrites);
        assert!(p.has(Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES));
    }

    #[test]
    fn owner_is_administrator() {
        let p = compute_base_permissions("g", "owner", "owner", &roles(&[("g", 0)]), &[]);
        assert!(p.administrator());
        assert!(p.can_manage_server());
        assert!(p.has(Permissions::MANAGE_MESSAGES));
    }

    #[test]
    fn everyone_role_applies_to_all_members() {
        let p = compute_base_permissions(
            "g",
            "owner",
            "user",
            &roles(&[("g", Permissions::MANAGE_MESSAGES)]),
            &[],
        );
        assert!(p.manage_messages());
        assert!(!p.manage_guild());
        assert!(p.can_manage_server());
    }

    #[test]
    fn member_roles_are_unioned() {
        let p = compute_base_permissions(
            "g",
            "owner",
            "user",
            &roles(&[("g", 0), ("r1", 1 << 10), ("r2", Permissions::MANAGE_ROLES)]),
            &["r1".to_owned(), "r2".to_owned()],
        );
        assert_eq!(p.bits(), (1 << 10) | Permissions::MANAGE_ROLES);
        assert!(p.manage_roles());
        assert!(!p.administrator());
    }

    #[test]
    fn unknown_roles_are_ignored() {
        let p = compute_base_permissions(
            "g",
            "owner",
            "user",
            &roles(&[("g", 0)]),
            &["nope".to_owned()],
        );
        assert_eq!(p.bits(), 0);
        assert!(!p.can_manage_server());
    }

    #[test]
    fn administrator_implies_everything() {
        let p = Permissions::from_bits(Permissions::ADMINISTRATOR);
        assert!(p.manage_guild() && p.manage_messages() && p.manage_roles() && p.create_events());
    }

    #[test]
    fn create_events_bit() {
        assert!(Permissions::from_bits(Permissions::CREATE_EVENTS).create_events());
        // 「イベントの管理」(1 << 33) では作成できない (#94 の実機確認より)
        assert!(!Permissions::from_bits(1 << 33).create_events());
    }
}
