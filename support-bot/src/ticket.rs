//! チケット (フォーラムの投稿) の状態と、フォーラムタグとの対応。
//!
//! 状態は DB とタグの両方に持つ。DB 側の変化 (利用者の再投稿による再オープン、`/ticket status`) は
//! 「解決済み」タグを付け外しして Discord に合わせ、Discord 側でタグが付け外しされたら DB に合わせる。
//! Bot 自身がタグを変えたときも ThreadUpdate が届くが、そのときは DB と一致しているので何も起きない。

use poise::serenity_prelude::{self as serenity, ForumTag, ForumTagId};

use crate::{data::Data, db};

/// 「解決済み」タグの名前。タグ ID はフォーラムごとに違うので、名前で引く
pub const RESOLVED_TAG: &str = "解決済み";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, poise::ChoiceParameter)]
pub enum Status {
    /// スタッフの返信待ち
    #[name = "対応待ち (スタッフの返信待ち)"]
    Open,
    /// 利用者の返信待ち
    #[name = "返信待ち (利用者の返信待ち)"]
    WaitingUser,
    #[name = "解決済み"]
    Resolved,
}

impl Status {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::WaitingUser => "waiting_user",
            Self::Resolved => "resolved",
        }
    }

    /// DB の値から戻す。CHECK 制約があるので未知の値は来ないが、来たら open として扱う
    pub fn from_db(value: &str) -> Self {
        match value {
            "waiting_user" => Self::WaitingUser,
            "resolved" => Self::Resolved,
            _ => Self::Open,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "対応待ち",
            Self::WaitingUser => "返信待ち",
            Self::Resolved => "解決済み",
        }
    }
}

/// 付いているタグ ID を名前に直す (フォーラムに無い ID は捨てる)
pub fn tag_names(available: &[ForumTag], applied: &[ForumTagId]) -> Vec<String> {
    applied
        .iter()
        .filter_map(|id| available.iter().find(|tag| tag.id == *id))
        .map(|tag| tag.name.clone())
        .collect()
}

pub fn has_resolved_tag(available: &[ForumTag], applied: &[ForumTagId]) -> bool {
    tag_names(available, applied)
        .iter()
        .any(|name| name == RESOLVED_TAG)
}

/// 状態に合わせたタグの集合。変える必要がなければ None。
/// フォーラムに「解決済み」タグが無ければ何もしない (DB だけで状態を持つ)
pub fn desired_tags(
    available: &[ForumTag],
    applied: &[ForumTagId],
    status: Status,
) -> Option<Vec<ForumTagId>> {
    let resolved = available.iter().find(|tag| tag.name == RESOLVED_TAG)?.id;
    let has = applied.contains(&resolved);
    match (status == Status::Resolved, has) {
        (true, false) => {
            // フォーラムのタグは 1 投稿 5 個まで。溢れるなら付けない (状態は DB に残る)
            if applied.len() >= 5 {
                return None;
            }
            let mut tags = applied.to_vec();
            tags.push(resolved);
            Some(tags)
        }
        (false, true) => Some(
            applied
                .iter()
                .copied()
                .filter(|id| *id != resolved)
                .collect(),
        ),
        _ => None,
    }
}

/// スレッドのタグを状態に合わせる。失敗しても DB の状態は正しいので、警告だけ残す
/// 呼び出し元が知っている状態ではなく、ロックを取ってから DB の最新の状態を読んで合わせる
/// (タグを編集する前に利用者が返信して再オープンしていたら、古い「解決済み」で上書きしないため)
pub async fn sync_tags(ctx: &serenity::Context, data: &Data, thread_id: serenity::ChannelId) {
    let mut edited = data.tag_sync.lock().await;
    let status = match db::thread_status(&data.pool, crate::record::snowflake(thread_id)).await {
        Ok(Some(status)) => status,
        Ok(None) => return,
        Err(error) => {
            tracing::warn!(%error, thread_id = thread_id.get(), "failed to read ticket status for tag sync");
            return;
        }
    };
    let thread = match thread_id.to_channel(ctx).await.map(|c| c.guild()) {
        Ok(Some(thread)) => thread,
        Ok(None) => return,
        Err(error) => {
            tracing::warn!(%error, thread_id = thread_id.get(), "failed to fetch thread for tag sync");
            return;
        }
    };
    let Some(parent_id) = thread.parent_id else {
        return;
    };
    let available = match forum_tags(ctx, thread.guild_id, parent_id).await {
        Some(tags) => tags,
        None => return,
    };
    let Some(tags) = desired_tags(&available, &thread.applied_tags, status) else {
        return;
    };
    let edit = serenity::EditThread::new().applied_tags(tags);
    match thread_id.edit_thread(ctx, edit).await {
        // この編集で届く ThreadUpdate を、event::sync_status_from_tags が Bot 自身のものとして読み飛ばす
        Ok(_) => {
            edited.insert(thread_id, status == Status::Resolved);
        }
        Err(error) => {
            tracing::warn!(%error, thread_id = thread_id.get(), "failed to update forum tags");
        }
    }
}

/// フォーラムに定義されたタグ。キャッシュに無ければ HTTP で取る
pub async fn forum_tags(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    forum_id: serenity::ChannelId,
) -> Option<Vec<ForumTag>> {
    if let Some(tags) =
        crate::record::cached_channel(ctx, guild_id, forum_id).map(|channel| channel.available_tags)
    {
        return Some(tags);
    }
    match forum_id.to_channel(ctx).await {
        Ok(channel) => channel.guild().map(|channel| channel.available_tags),
        Err(error) => {
            tracing::warn!(%error, forum_id = forum_id.get(), "failed to fetch forum tags");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(id: u64, name: &str) -> ForumTag {
        serde_json::from_value(serde_json::json!({
            "id": id.to_string(),
            "name": name,
            "moderated": false,
            "emoji_id": null,
            "emoji_name": null,
        }))
        .unwrap()
    }

    fn id(id: u64) -> ForumTagId {
        ForumTagId::new(id)
    }

    #[test]
    fn status_round_trips_through_db_value() {
        for status in [Status::Open, Status::WaitingUser, Status::Resolved] {
            assert_eq!(Status::from_db(status.as_db()), status);
        }
    }

    #[test]
    fn names_applied_tags() {
        let available = [tag(1, "不具合"), tag(2, RESOLVED_TAG)];
        assert_eq!(tag_names(&available, &[id(2), id(9)]), [RESOLVED_TAG]);
        assert!(has_resolved_tag(&available, &[id(1), id(2)]));
        assert!(!has_resolved_tag(&available, &[id(1)]));
    }

    #[test]
    fn adds_and_removes_resolved_tag() {
        let available = [tag(1, "不具合"), tag(2, RESOLVED_TAG)];
        assert_eq!(
            desired_tags(&available, &[id(1)], Status::Resolved),
            Some(vec![id(1), id(2)])
        );
        assert_eq!(
            desired_tags(&available, &[id(1), id(2)], Status::Open),
            Some(vec![id(1)])
        );
        assert_eq!(
            desired_tags(&available, &[id(1), id(2)], Status::Resolved),
            None
        );
        assert_eq!(
            desired_tags(&available, &[id(1)], Status::WaitingUser),
            None
        );
    }

    #[test]
    fn leaves_tags_alone_without_resolved_tag_or_room() {
        assert_eq!(desired_tags(&[tag(1, "質問")], &[], Status::Resolved), None);
        let available = [tag(2, RESOLVED_TAG)];
        let full = [id(3), id(4), id(5), id(6), id(7)];
        assert_eq!(desired_tags(&available, &full, Status::Resolved), None);
    }
}
