use ircgm::prelude::*;

use crate::role::{DeathCtx, GameSet, Role, RoleInfo, Team, VoteDefense};

pub struct DefenderEr;
pub static DEFENDER_ER: DefenderEr = DefenderEr;

impl Role for DefenderEr {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "defender_er",
            "Defender-er",
            GameSet::SuperVillains,
            Team::Village,
            "Sleep at night. Whoever you point at during the Vote CANNOT be caught (if they would die, the next-highest vote-getter with 2+ votes is caught instead)!",
        )
        .aliases(&["defenderer", "defender"])
    }

    fn vote_defense(&self, seat: usize, ctx: &DeathCtx<'_>) -> Option<VoteDefense> {
        let target = ctx.votes[seat]?;
        Some(VoteDefense::immune(
            target,
            true,
            irc_format!(
                "🛡️🦸 {} ({}) defended {} from being caught!",
                ctx.roster.seats[seat].user.nick().bold(),
                self.info().styled_name(),
                ctx.roster.seats[target].user.nick().bold(),
            ),
        ))
    }
}

#[cfg(test)]
#[path = "defender_er_test.rs"]
mod test;
