//! Bot のプレゼンス (ステータス表示) を10秒ごとに切り替える (旧 `tasks/presence.rs` 相当)。
//!
//! 旧 Bot は廃止済みのプレフィックスコマンド `cal help` も案内していたが、
//! `bot/README.md` の移行状況どおり廃止済みなのでここでは出さない。

use std::time::Duration;

use poise::serenity_prelude::{self as serenity, ActivityData, OnlineStatus};

const INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy)]
enum Presence {
    ShowHelp,
    NumServers,
    ServerUrl,
}

impl Presence {
    fn next(self) -> Self {
        match self {
            Self::ShowHelp => Self::NumServers,
            Self::NumServers => Self::ServerUrl,
            Self::ServerUrl => Self::ShowHelp,
        }
    }

    fn activity(self, ctx: &serenity::Context) -> ActivityData {
        match self {
            Self::ShowHelp => ActivityData::watching("/help"),
            Self::NumServers => {
                let num_servers = ctx.cache.guilds().len();
                ActivityData::watching(format!("{num_servers} servers"))
            }
            Self::ServerUrl => ActivityData::listening("discalendar.app"),
        }
    }
}

pub async fn run_loop(ctx: serenity::Context) {
    let mut state = Presence::ShowHelp;
    loop {
        ctx.set_presence(Some(state.activity(&ctx)), OnlineStatus::Online);
        state = state.next();
        tokio::time::sleep(INTERVAL).await;
    }
}
