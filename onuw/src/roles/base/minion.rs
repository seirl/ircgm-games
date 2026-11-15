use ircgm::prelude::*;

use crate::role::{
    BoxFuture, GameSet, GroupEffect, GroupPromptCtx, Role, RoleInfo, RoleTag, Team, Wake,
};

pub struct Minion;
pub static MINION: Minion = Minion;

impl Role for Minion {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "minion",
            "Minion",
            GameSet::Base,
            Team::Werewolf,
            "Wake to learn who the Werewolves are (they do not know who you are). Win if Werewolves survive!",
        )
        .minion()
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::group(130, Self::wake_group))
    }
}

impl Minion {
    fn wake_group<'a>(
        active_seats: Vec<usize>,
        ctx: GroupPromptCtx<'a>,
    ) -> BoxFuture<'a, Result<GroupEffect>> {
        Box::pin(async move {
            let mut wolves = ctx.seats_with_tag(RoleTag::WolfPack);
            for dw in ctx.seats_with_tag(RoleTag::DreamWolfThumb) {
                if !wolves.contains(&dw) {
                    wolves.push(dw);
                }
            }
            wolves.sort_unstable();

            for &m_seat in &active_seats {
                if wolves.is_empty() {
                    let goal = if ctx.roster.deck_has_tanner_hazard() {
                        "Survive and get another non-Tanner player killed to win!"
                    } else {
                        "Survive and get another player killed to win!"
                    };
                    ctx.dm(
                        m_seat,
                        irc_format!(
                            "🗡️ Minion: {}! {goal}",
                            "NO players hold Werewolf cards".bold().yellow()
                        ),
                    );
                } else {
                    ctx.dm(
                        m_seat,
                        irc_format!(
                            "🗡️ Minion: The Werewolves are {} (they do not know who you are!).",
                            ctx.roster.format_indices(&wolves).bold().red()
                        ),
                    );
                }
            }

            Ok(GroupEffect::new(move |_board, rctx| {
                let names = rctx.roster.format_indices(&active_seats);
                let styled = MINION.info().styled_name();
                if wolves.is_empty() {
                    rctx.record(irc_format!(
                        "🗡️ {styled} ({}) saw no Werewolves in play.",
                        names.bold(),
                    ));
                } else {
                    let wolf_names = rctx.roster.format_indices(&wolves);
                    rctx.record(irc_format!(
                        "🗡️ {styled} ({}) saw Werewolf pack: {}.",
                        names.bold(),
                        wolf_names.bold().red(),
                    ));
                }
            }))
        })
    }
}

#[cfg(test)]
#[path = "minion_test.rs"]
mod test;
