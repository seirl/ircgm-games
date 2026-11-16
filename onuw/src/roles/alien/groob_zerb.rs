use ircgm::prelude::*;

use super::alien::ALIEN_PACK_STEP;
use crate::board::{BoardCards, SeatSet};
use crate::role::{
    EffectiveAlignment, GameSet, GroupEffect, GroupPromptCtx, NightFaction, Role, RoleInfo,
    SpecialVictory, Team, VictoryCtx, WakeStep,
};

pub struct Groob;
pub static GROOB: Groob = Groob;

pub struct Zerb;
pub static ZERB: Zerb = Zerb;

pub(super) fn both_groob_and_zerb_in_play(board: &BoardCards) -> bool {
    !board.active_holders(&GROOB).is_empty() && !board.active_holders(&ZERB).is_empty()
}

impl Role for Groob {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "groob",
            "Groob",
            GameSet::Alien,
            Team::Independent,
            "Rival Alien: wake with the Aliens; if Zerb is in play, win ONLY if you survive and Zerb dies (otherwise win with the Aliens)!",
        )
        .night_faction(NightFaction::Alien)
        .threat_team(Team::Alien)
        .anarchy_bundle("groob_zerb")
        .non_village_labels(&["Alien"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                ALIEN_PACK_STEP,
                WakeStep::group(&GROOB, 114, |seats, ctx| {
                    Box::pin(Self::wake_group(seats, ctx))
                }),
            ]
        }
    }

    fn adjust_alignment(
        &self,
        _seat: usize,
        board: &BoardCards,
        mut cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        cur.team = if both_groob_and_zerb_in_play(board) {
            Team::Independent
        } else {
            Team::Alien
        };
        cur
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        if !both_groob_and_zerb_in_play(ctx.board) || ctx.is_dead(seat) {
            return None;
        }
        ctx.active_holders(&ZERB)
            .intersects(ctx.dead_seats)
            .then_some(SpecialVictory::independent())
    }
}

impl Groob {
    async fn wake_group(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let zerb_seats = ctx.seats_with_role(&ZERB);
        let zerb_info = if zerb_seats.is_empty() {
            for s in active_seats {
                ctx.dm(
                    s,
                    "👾 Groob: Zerb is in the center — you are on the regular Alien team tonight!",
                );
            }
            "in the center".to_owned()
        } else {
            let names = ctx.roster.format_seats(zerb_seats);
            for s in active_seats {
                ctx.dm(
                    s,
                    irc_format!(
                        "👾 Groob: Your mortal nemesis Zerb is {} — survive and get Zerb killed to win!",
                        names.as_str().bold().red(),
                    ),
                );
            }
            names
        };
        Ok(GroupEffect::new(move |_board, rctx| {
            let names = rctx.roster.format_seats(active_seats);
            rctx.record(irc_format!(
                "👾 {} ({}) spotted nemesis Zerb ({}).",
                GROOB.info().styled_name(),
                names.bold(),
                zerb_info.as_str().bold(),
            ));
        }))
    }
}

impl Role for Zerb {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "zerb",
            "Zerb",
            GameSet::Alien,
            Team::Independent,
            "Rival Alien: wake with the Aliens; if Groob is in play, win ONLY if you survive and Groob dies (otherwise win with the Aliens)!",
        )
        .night_faction(NightFaction::Alien)
        .threat_team(Team::Alien)
        .anarchy_bundle("groob_zerb")
        .non_village_labels(&["Alien"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                ALIEN_PACK_STEP,
                WakeStep::group(&ZERB, 115, |seats, ctx| {
                    Box::pin(Self::wake_group(seats, ctx))
                }),
            ]
        }
    }

    fn adjust_alignment(
        &self,
        _seat: usize,
        board: &BoardCards,
        mut cur: EffectiveAlignment,
    ) -> EffectiveAlignment {
        cur.team = if both_groob_and_zerb_in_play(board) {
            Team::Independent
        } else {
            Team::Alien
        };
        cur
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        if !both_groob_and_zerb_in_play(ctx.board) || ctx.is_dead(seat) {
            return None;
        }
        ctx.active_holders(&GROOB)
            .intersects(ctx.dead_seats)
            .then_some(SpecialVictory::independent())
    }
}

impl Zerb {
    async fn wake_group(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let groob_seats = ctx.seats_with_role(&GROOB);
        let groob_info = if groob_seats.is_empty() {
            for s in active_seats {
                ctx.dm(
                    s,
                    "👾 Zerb: Groob is in the center — you are on the regular Alien team tonight!",
                );
            }
            "in the center".to_owned()
        } else {
            let names = ctx.roster.format_seats(groob_seats);
            for s in active_seats {
                ctx.dm(
                    s,
                    irc_format!(
                        "👾 Zerb: Your mortal nemesis Groob is {} — survive and get Groob killed to win!",
                        names.as_str().bold().red(),
                    ),
                );
            }
            names
        };
        Ok(GroupEffect::new(move |_board, rctx| {
            let names = rctx.roster.format_seats(active_seats);
            rctx.record(irc_format!(
                "👾 {} ({}) spotted nemesis Groob ({}).",
                ZERB.info().styled_name(),
                names.bold(),
                groob_info.as_str().bold(),
            ));
        }))
    }
}
