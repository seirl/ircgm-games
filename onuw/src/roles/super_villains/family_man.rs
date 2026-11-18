use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardKey, BoardMut, Roster, SeatSet};
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, SpecialVictory, Team, VictoryCtx, WakeStep};

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
    const ACCESS: u16 = BoardAccess::ROLE_STATE;

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
            &[WakeStep::passive(&FAMILY_MAN, Self::notify_holder)
                .eyes_closed()
                .with_tier_hooks(Self::init_tier, Self::finish_tier)
                .writes(ActiveFamilyShape::ACCESS | BoardAccess::PUBLIC_ANNOUNCE)]
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
    fn init_tier(board: &mut BoardMut<'_>, roster: &Roster) {
        if board.get_key::<ActiveFamilyShape>().0.is_none() {
            board.set_key(ActiveFamilyShape(Some(FamilyShape::roll(roster.len()))));
        }
    }

    fn notify_holder(
        seat: usize,
        actor_label: &str,
        board: &mut BoardMut<'_>,
        rctx: &mut ResolveCtx<'_>,
    ) {
        let shape = board
            .get_key::<ActiveFamilyShape>()
            .0
            .unwrap_or(FamilyShape::BothNeighbors);
        let fam = shape.family_seats(rctx.roster.len(), seat);
        let names = rctx.roster.format_seats(fam);
        rctx.dm(
            seat,
            irc_format!(
                "👨‍👩‍👧 {actor_label}: If you still hold Family Man, your family is {} — keep ALL of them alive to win!",
                names.as_str().bold().orange(),
            ),
        );
    }

    fn finish_tier(board: &mut BoardMut<'_>, rctx: &mut ResolveCtx<'_>) {
        let shape = board
            .get_key::<ActiveFamilyShape>()
            .0
            .unwrap_or(FamilyShape::BothNeighbors);
        rctx.announce(irc_format!(
            "👨‍👩‍👧 {} Whoever holds {} has {} in their family ({}) — keep the whole family alive!",
            "Family Man:".bold().orange(),
            FAMILY_MAN.info().styled_name(),
            shape.description().bold().yellow(),
            rctx.roster.format_table_circle(),
        ));
        rctx.record(irc_format!(
            "👨‍👩‍👧 Family Man shape rolled: {}.",
            shape.description().bold().orange(),
        ));
    }
}

#[cfg(test)]
#[path = "family_man_test.rs"]
mod test;
