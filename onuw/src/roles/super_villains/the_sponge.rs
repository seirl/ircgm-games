use ircgm::prelude::*;

use crate::role::{DeathCtx, GameSet, Role, RoleInfo, Team, VoteDefense};

pub struct TheSponge;
pub static THE_SPONGE: TheSponge = TheSponge;

impl Role for TheSponge {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "the_sponge",
            "The Sponge",
            GameSet::SuperVillains,
            Team::Village,
            "Sleep at night. Votes cast for The Sponge do not count — if you get the most votes, the next-highest vote-getter (>= 2 votes) is caught instead!",
        )
        .aliases(&["sponge"])
    }

    fn vote_defense(&self, seat: usize, ctx: &DeathCtx<'_>) -> Option<VoteDefense> {
        // Rule (Bonus Roles BP4 — The Sponge): "Players who point at the
        // Sponge do not count." Like Prince, only vote tallies are ignored
        // (`blocks_hunter_shot = false`), not Hunter death shots.
        Some(VoteDefense::immune(
            seat,
            false,
            irc_format!(
                "🧽 {} ({}) absorbed the votes and cannot be executed!",
                ctx.roster.seats[seat].user.nick().bold(),
                self.info().styled_name(),
            ),
        ))
    }
}

#[cfg(test)]
#[path = "the_sponge_test.rs"]
mod test;
