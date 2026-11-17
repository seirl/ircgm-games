use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardKey, SeatSet};
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, Role, RoleInfo, SpecialVictory, Team, VictoryCtx,
    WakeStep,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FamilyShape {
    Left1,
    Right1,
    BothNeighbors,
}

impl FamilyShape {
    fn roll(player_count: usize) -> Self {
        if player_count >= 4 {
            match fastrand::u8(..3) {
                0 => Self::Left1,
                1 => Self::Right1,
                _ => Self::BothNeighbors,
            }
        } else if fastrand::bool() {
            Self::Left1
        } else {
            Self::Right1
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Left1 => "the player immediately to their LEFT",
            Self::Right1 => "the player immediately to their RIGHT",
            Self::BothNeighbors => "both immediate LEFT & RIGHT neighbors",
        }
    }

    pub(crate) fn family_seats(self, n: usize, seat: usize) -> SeatSet {
        let left = (seat + n - 1) % n;
        let right = (seat + 1) % n;
        let mut set = SeatSet::single(seat);
        match self {
            Self::Left1 => {
                set.insert(left);
            }
            Self::Right1 => {
                set.insert(right);
            }
            Self::BothNeighbors => {
                set.insert(left);
                set.insert(right);
            }
        }
        set
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ActiveFamilyShape(pub Option<FamilyShape>);

impl BoardKey for ActiveFamilyShape {
    const ACCESS: u16 = BoardAccess::CUSTOM_1;

    fn init() -> Self {
        Self(None)
    }
}

pub struct FamilyMan;
pub static FAMILY_MAN: FamilyMan = FamilyMan;

impl Role for FamilyMan {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "family_man",
            "Family Man",
            GameSet::SuperVillains,
            Team::Independent,
            "Sleep at night! At the end of Night, the announcer declares which adjacent neighbors belong to Family Man's family — win ONLY if you and every member of your family survive!",
        )
        .aliases(&["familyman", "family"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::group(&FAMILY_MAN, 196, |seats, ctx| {
                    Box::pin(Self::wake(seats, ctx))
                })
                .eyes_closed()
                .run_when_unheld()
                .writes(ActiveFamilyShape::ACCESS),
            ]
        }
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        let shape = ctx
            .board
            .get_key::<ActiveFamilyShape>()
            .0
            .unwrap_or(FamilyShape::BothNeighbors);
        let family = shape.family_seats(ctx.roster.len(), seat);
        family
            .is_disjoint(ctx.dead_seats)
            .then_some(SpecialVictory::independent())
    }
}

impl FamilyMan {
    async fn wake(active_seats: SeatSet, _ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        Ok(GroupEffect::new(move |board, rctx| {
            let n = rctx.roster.len();
            let shape = match board.get_key::<ActiveFamilyShape>().0 {
                Some(s) => s,
                None => {
                    let rolled = FamilyShape::roll(n);
                    board.set_key(ActiveFamilyShape(Some(rolled)));
                    rolled
                }
            };

            rctx.announce(irc_format!(
                "👨‍👩‍👧 {} Whoever holds {} has {} in their family ({}) — keep the whole family alive!",
                "Family Man:".bold().orange(),
                FAMILY_MAN.info().styled_name(),
                shape.description().bold().yellow(),
                rctx.roster.format_table_circle(),
            ));

            for s in active_seats {
                let fam = shape.family_seats(n, s);
                let names = rctx.roster.format_seats(fam);
                rctx.dm(
                    s,
                    irc_format!(
                        "👨‍👩‍👧 Family Man: If you still hold Family Man, your family is {} — keep ALL of them alive to win!",
                        names.as_str().bold().orange(),
                    ),
                );
            }

            rctx.record(irc_format!(
                "👨‍👩‍👧 Family Man shape rolled: {}.",
                shape.description().bold().orange(),
            ));
        }))
    }
}

#[cfg(test)]
#[path = "family_man_test.rs"]
mod test;
