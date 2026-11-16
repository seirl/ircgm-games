use ircgm::prelude::*;

use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, NightFaction, Role, RoleInfo, Team, WakeStep,
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

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::group(&MINION, 130, |seats, ctx| {
                Box::pin(Self::wake_group(seats, ctx))
            })]
        }
    }
}

impl Minion {
    async fn wake_group(active_seats: Vec<usize>, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let has_tanner = ctx.roster.deck_has(&super::tanner::TANNER);
        let wolves = ctx.seats_with_faction(NightFaction::Werewolf);
        for &m_seat in &active_seats {
            if wolves.is_empty() {
                let goal = if has_tanner {
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
    }
}

#[cfg(test)]
#[path = "minion_test.rs"]
mod test;
