use ircgm::prelude::*;

use crate::role::{DeathCtx, GameSet, Role, RoleInfo, Team, VoteDefense};

pub struct Bodyguard;
pub static BODYGUARD: Bodyguard = Bodyguard;

impl Role for Bodyguard {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "bodyguard",
            "Bodyguard",
            GameSet::Daybreak,
            Team::Village,
            "Sleep at night. Whoever you point at during the Day vote CANNOT die (the next-highest vote-getter with 2+ votes dies instead!).",
        )
        .aliases(&["bg"])
    }

    fn vote_defense(&self, seat: usize, ctx: &DeathCtx<'_>) -> Option<VoteDefense> {
        let target = ctx.votes[seat];
        Some(VoteDefense::immune(
            target,
            true,
            irc_format!(
                "🛡️ {} ({}) shielded {} from execution!",
                ctx.roster.seats[seat].user.nick().bold(),
                self.info().styled_name(),
                ctx.roster.seats[target].user.nick().bold(),
            ),
        ))
    }
}

#[cfg(test)]
#[path = "bodyguard_test.rs"]
mod test;
