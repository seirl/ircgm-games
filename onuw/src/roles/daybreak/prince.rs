use ircgm::prelude::*;

use crate::role::{DeathCtx, GameSet, Role, RoleInfo, Team, VoteProtection};

pub struct Prince;
pub static PRINCE: Prince = Prince;

impl Role for Prince {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "prince",
            "Prince",
            GameSet::Daybreak,
            Team::Village,
            "Sleep at night. Royal immunity: you CANNOT die! Any votes against you deflect to the next-highest vote-getter (>= 2 votes).",
        )
    }

    fn vote_protection(&self, seat: usize, ctx: &DeathCtx<'_>) -> Option<VoteProtection> {
        Some(VoteProtection {
            protected_seat: seat,
            blocks_hunter_shot: false,
            announcement: irc_format!(
                "👑 {} ({}) holds royal immunity and cannot be executed!",
                ctx.roster.seats[seat].user.nick().bold(),
                self.info().styled_name(),
            ),
        })
    }
}

#[cfg(test)]
#[path = "prince_test.rs"]
mod test;
