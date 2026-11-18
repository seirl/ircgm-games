use ircgm::prelude::*;

use crate::artifacts::Artifact;
use crate::board::{BoardMut, SeatSet};
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, SpecialVictory, Team, VictoryCtx, WakeStep};
use crate::roles::base::tanner::TANNER;

pub struct ApprenticeTanner;
pub static APPRENTICE_TANNER: ApprenticeTanner = ApprenticeTanner;

impl Role for ApprenticeTanner {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "apprentice_tanner",
            "Apprentice Tanner",
            GameSet::Vampire,
            Team::Tanner,
            "Wake to see who the Tanner is: win ONLY if the Tanner dies! If no Tanner is in play, win ONLY if YOU die!",
        )
        .aliases(&["apptanner", "at"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WakeStep::passive(&APPRENTICE_TANNER, Self::inspect)] }
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        let cudgel_holders: SeatSet = (0..ctx.roster.len())
            .filter(|&i| ctx.board.artifact(i) == Some(Artifact::CudgelOfTheTanner))
            .collect();
        let tanners = (ctx.active_holders(&TANNER) | cudgel_holders).without(seat);
        let won = if tanners.is_empty() {
            ctx.is_dead(seat)
        } else {
            tanners.intersects(ctx.dead_seats)
        };
        won.then_some(SpecialVictory::independent_blocking_monsters())
    }
}

impl ApprenticeTanner {
    fn inspect(
        seat: usize,
        actor_label: &str,
        _board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let tanners = ctx.seats_with_role(&TANNER).without(seat);
        let actor_nick = ctx.nick(seat);
        if tanners.is_empty() {
            ctx.dm(
                seat,
                irc_format!(
                    "🪢 {actor_label}: {}! You win ONLY if YOU die!",
                    "NO player holds the Tanner".bold().yellow(),
                ),
            );
            ctx.record(irc_format!(
                "🪢 {actor_label} ({}) saw no Tanner in play.",
                actor_nick.bold(),
            ));
        } else {
            let tanner_names = ctx.roster.format_seats(tanners);
            ctx.dm(
                seat,
                irc_format!(
                    "🪢 {actor_label}: The Tanner is {} — get the Tanner killed to win!",
                    tanner_names.as_str().bold().orange(),
                ),
            );
            ctx.record(irc_format!(
                "🪢 {actor_label} ({}) saw Tanner: {}.",
                actor_nick.bold(),
                tanner_names.bold().orange(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "apprentice_tanner_test.rs"]
mod test;
