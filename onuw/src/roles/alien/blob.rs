use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardKey, BoardMut, Roster, SeatSet};
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, SpecialVictory, Team, VictoryCtx, WakeStep};

/// Per-game randomized absorption shape announced publicly at `#13 Blob`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlobShape {
    Left1,
    Right1,
    LeftAndRight,
    Left2,
    Right2,
}

impl BlobShape {
    fn roll(player_count: usize) -> Self {
        let mut pool = vec![Self::Left1, Self::Right1];
        if player_count >= 4 {
            pool.push(Self::LeftAndRight);
        }
        if player_count >= 5 {
            pool.push(Self::Left2);
            pool.push(Self::Right2);
        }
        pool[fastrand::usize(..pool.len())]
    }

    fn description(self) -> &'static str {
        match self {
            Self::Left1 => "1 player to their LEFT",
            Self::Right1 => "1 player to their RIGHT",
            Self::LeftAndRight => "both immediate LEFT & RIGHT neighbors",
            Self::Left2 => "2 players to their LEFT",
            Self::Right2 => "2 players to their RIGHT",
        }
    }

    pub(crate) fn absorbed_seats(self, n: usize, seat: usize) -> SeatSet {
        let left1 = (seat + n - 1) % n;
        let right1 = (seat + 1) % n;
        let mut set = SeatSet::single(seat);
        match self {
            Self::Left1 => {
                set.insert(left1);
            }
            Self::Right1 => {
                set.insert(right1);
            }
            Self::LeftAndRight => {
                set.insert(left1);
                set.insert(right1);
            }
            Self::Left2 => {
                set.insert(left1);
                set.insert((seat + n - 2) % n);
            }
            Self::Right2 => {
                set.insert(right1);
                set.insert((seat + 2) % n);
            }
        }
        set
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ActiveBlobShape(pub Option<BlobShape>);

impl BoardKey for ActiveBlobShape {
    const ACCESS: u16 = BoardAccess::CUSTOM_1;

    fn init() -> Self {
        Self(None)
    }
}

pub struct Blob;
pub static BLOB: Blob = Blob;

impl Role for Blob {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "blob",
            "Blob",
            GameSet::Alien,
            Team::Independent,
            "Keep your eyes closed! At the end of Night, the Mother Ship announces which adjacent neighbors are absorbed into the Blob — win ONLY if you and every absorbed player survive!",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::passive(&BLOB, Self::notify_holder)
                .eyes_closed()
                .with_tier_hooks(Self::init_tier, Self::finish_tier)
                .writes(ActiveBlobShape::ACCESS | BoardAccess::PUBLIC_ANNOUNCE)]
        }
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        let shape = ctx
            .board
            .get_key::<ActiveBlobShape>()
            .0
            .unwrap_or(BlobShape::Left1);
        let absorbed = shape.absorbed_seats(ctx.roster.len(), seat);
        absorbed
            .is_disjoint(ctx.dead_seats)
            .then_some(SpecialVictory::independent())
    }
}

impl Blob {
    fn init_tier(board: &mut BoardMut<'_>, roster: &Roster) {
        if board.get_key::<ActiveBlobShape>().0.is_none() {
            board.set_key(ActiveBlobShape(Some(BlobShape::roll(roster.len()))));
        }
    }

    fn notify_holder(
        seat: usize,
        actor_label: &str,
        board: &mut BoardMut<'_>,
        rctx: &mut ResolveCtx<'_>,
    ) {
        let shape = board
            .get_key::<ActiveBlobShape>()
            .0
            .unwrap_or(BlobShape::Left1);
        let absorbed = shape.absorbed_seats(rctx.roster.len(), seat);
        let names = rctx.roster.format_seats(absorbed);
        rctx.dm(
            seat,
            irc_format!(
                "🟢 {actor_label}: If you still hold the Blob, your absorbed cluster is {} — keep ALL of them alive to win!",
                names.as_str().bold().green(),
            ),
        );
    }

    fn finish_tier(board: &mut BoardMut<'_>, rctx: &mut ResolveCtx<'_>) {
        let shape = board
            .get_key::<ActiveBlobShape>()
            .0
            .unwrap_or(BlobShape::Left1);
        rctx.announce(irc_format!(
            "🟢 {} Whoever holds the {} is merged with {} ({}) — keep ALL absorbed players alive!",
            "Blob Absorption:".bold().green(),
            BLOB.info().styled_name(),
            shape.description().bold().yellow(),
            rctx.roster.format_table_circle(),
        ));
        rctx.record(irc_format!(
            "🟢 Blob absorption rolled: {}.",
            shape.description().bold().green(),
        ));
    }
}

#[cfg(test)]
#[path = "blob_test.rs"]
mod test;
