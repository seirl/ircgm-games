use ircgm::prelude::*;

use crate::role::{DeathCtx, GameSet, Role, RoleInfo, Team, VoteDefense};

pub struct Prince;
pub static PRINCE: Prince = Prince;

impl Role for Prince {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "prince",
            "Prince",
            GameSet::Daybreak,
            Team::Village,
            "Sleep at night. Royal immunity: votes against you do not count and deflect to the next-highest vote-getter (>= 2 votes)!",
        )
    }

    fn vote_defense(&self, seat: usize, ctx: &DeathCtx<'_>) -> Option<VoteDefense> {
        Some(VoteDefense::immune(
            seat,
            false,
            irc_format!(
                "👑 {} ({}) holds royal immunity and cannot be executed!",
                ctx.roster.seats[seat].user.nick().bold(),
                self.info().styled_name(),
            ),
        ))
    }
}

#[cfg(test)]
#[path = "prince_test.rs"]
mod test;
