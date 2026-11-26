use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use poise::serenity_prelude::{ChannelId, RoleId};
use sqlx::PgPool;

use crate::config::Settings;

/// poise のユーザーデータ。全コマンド・イベントハンドラから `ctx.data()` で参照できる
#[derive(Debug, Clone)]
pub struct Data {
    pub pool: PgPool,
    pub settings: Arc<Settings>,
    /// 定期タスクと取りこぼしの補完を起動済みかどうか (再接続で Ready が何度届いても一度だけ起動する)
    pub tasks_started: Arc<AtomicBool>,
}

impl Data {
    pub fn mark_tasks_started(&self) -> bool {
        self.tasks_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn is_record_channel(&self, channel_id: ChannelId) -> bool {
        self.settings.record_channel_ids.contains(&channel_id)
    }

    pub fn is_staff(&self, roles: &[RoleId]) -> bool {
        is_staff(&self.settings.staff_role_ids, roles)
    }
}

pub fn is_staff(staff_role_ids: &std::collections::HashSet<RoleId>, roles: &[RoleId]) -> bool {
    roles.iter().any(|role| staff_role_ids.contains(role))
}

/// コマンドハンドラが受け取るコンテキスト
pub type Context<'a> = poise::Context<'a, Data, crate::error::BotError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staff_is_detected_by_any_role() {
        let staff = [RoleId::new(10), RoleId::new(20)].into_iter().collect();
        assert!(is_staff(&staff, &[RoleId::new(1), RoleId::new(20)]));
        assert!(!is_staff(&staff, &[RoleId::new(1)]));
        assert!(!is_staff(&staff, &[]));
    }
}
