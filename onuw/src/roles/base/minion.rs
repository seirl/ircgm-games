use ircgm::prelude::*;

use crate::role::{GameSet, GroupEffect, GroupPromptCtx, Role, RoleInfo, RoleTag, Team, Wake};

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
        Some(Wake::group(130, |seats, ctx| {
            Box::pin(Self::wake_group(seats, ctx))
        }))
    }
}

impl Minion {
    async fn wake_group(active_seats: Vec<usize>, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let has_tanner_hazard = ctx.roster.deck_has_tanner_hazard();
        Ok(GroupEffect::new(move |board, rctx| {
            let mut wolves: Vec<usize> = (0..rctx.roster.len())
                .filter(|&i| {
                    let wr = rctx.waking_roles[i];
                    wr.tags().contains(&RoleTag::WolfPack)
                        || wr.tags().contains(&RoleTag::DreamWolfThumb)
                        || board.players[i].became.is_some_and(|b| {
                            b.tags().contains(&RoleTag::WolfPack)
                                || b.tags().contains(&RoleTag::DreamWolfThumb)
                        })
                })
                .collect();
            wolves.sort_unstable();

            for &m_seat in &active_seats {
                if wolves.is_empty() {
                    let goal = if has_tanner_hazard {
                        "Survive and get another non-Tanner player killed to win!"
                    } else {
                        "Survive and get another player killed to win!"
                    };
                    rctx.dm(
                        m_seat,
                        irc_format!(
                            "🗡️ Minion: {}! {goal}",
                            "NO players hold Werewolf cards".bold().yellow()
                        ),
                    );
                } else {
                    rctx.dm(
                        m_seat,
                        irc_format!(
                            "🗡️ Minion: The Werewolves are {} (they do not know who you are!).",
                            rctx.roster.format_indices(&wolves).bold().red()
                        ),
                    );
                }
            }

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
    }
}

#[cfg(test)]
#[path = "minion_test.rs"]
mod test;
