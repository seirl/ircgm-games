use ircgm::prelude::*;

use crate::artifacts::Artifact;
use crate::board::SeatSet;
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, Role, RoleInfo, SpecialVictory, Team, VictoryCtx,
    WakeStep,
};
use crate::roles::base::tanner::TANNER;

pub struct ApprenticeTanner;
pub static APPRENTICE_TANNER: ApprenticeTanner = ApprenticeTanner;

impl Role for ApprenticeTanner {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "apprentice_tanner",
            "Apprentice Tanner",
            GameSet::Vampire,
            Team::Independent,
            "Wake to see who the Tanner is: win ONLY if the Tanner dies! If no Tanner is in play, win ONLY if YOU die!",
        )
        .aliases(&["apptanner", "at"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::group(&APPRENTICE_TANNER, |seats, ctx| {
                    Box::pin(Self::wake_group(seats, ctx))
                }),
            ]
        }
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
    async fn wake_group(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let tanners = ctx.seats_with_role(&TANNER);
        for seat in active_seats {
            let peers = active_seats.without(seat);
            let peer_note = if peers.is_empty() {
                String::new()
            } else {
                format!(
                    " (fellow Apprentice Tanner: {})",
                    ctx.roster.format_seats(peers)
                )
            };
            if tanners.is_empty() {
                ctx.dm(
                    seat,
                    irc_format!(
                        "🪢 Apprentice Tanner: {}{peer_note}! You win ONLY if YOU die!",
                        "NO player holds the Tanner".bold().yellow(),
                    ),
                );
            } else {
                ctx.dm(
                    seat,
                    irc_format!(
                        "🪢 Apprentice Tanner: The Tanner is {}{peer_note} — get the Tanner killed to win!",
                        ctx.roster.format_seats(tanners).bold().orange(),
                    ),
                );
            }
        }

        Ok(GroupEffect::new(move |_board, rctx| {
            let names = rctx.roster.format_seats(active_seats);
            let styled = APPRENTICE_TANNER.info().styled_name();
            if tanners.is_empty() {
                rctx.record(irc_format!(
                    "🪢 {styled} ({}) saw no Tanner in play.",
                    names.bold(),
                ));
            } else {
                let tanner_names = rctx.roster.format_seats(tanners);
                rctx.record(irc_format!(
                    "🪢 {styled} ({}) saw Tanner: {}.",
                    names.bold(),
                    tanner_names.bold().orange(),
                ));
            }
        }))
    }
}

#[cfg(test)]
#[path = "apprentice_tanner_test.rs"]
mod test;
