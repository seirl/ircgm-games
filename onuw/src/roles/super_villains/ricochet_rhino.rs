use ircgm::prelude::*;

use crate::role::{DeathCtx, GameSet, Role, RoleInfo, Team, VoteDefense};

pub struct RicochetRhino;
pub static RICOCHET_RHINO: RicochetRhino = RicochetRhino;

impl Role for RicochetRhino {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "ricochet_rhino",
            "Ricochet Rhino",
            GameSet::SuperVillains,
            Team::Village,
            "Sleep at night. If you receive enough votes to be caught, you survive and the vote ricochets — catching whoever YOU pointed at instead!",
        )
        .aliases(&["rhino", "ricochet"])
    }

    fn vote_defense(&self, seat: usize, ctx: &DeathCtx<'_>) -> Option<VoteDefense> {
        let target = ctx.votes[seat];
        if target == seat {
            return None;
        }
        Some(VoteDefense::ricochet(
            seat,
            target,
            irc_format!(
                "🦏💥 Votes bounced off {} ({}) and ricocheted onto {}!",
                ctx.roster.seats[seat].user.nick().bold(),
                self.info().styled_name(),
                ctx.roster.seats[target].user.nick().bold(),
            ),
        ))
    }
}

#[cfg(test)]
#[path = "ricochet_rhino_test.rs"]
mod test;
