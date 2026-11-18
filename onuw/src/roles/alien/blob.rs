use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardKey, BoardMut, Roster, SeatSet};
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, SpecialVictory, Team, VictoryCtx, WakeStep};

/// Per-game randomized absorption shape announced publicly at `#13 Blob`.
///
/// Official *One Night Ultimate Alien* player-count absorption table (contiguous adjacent players
/// absorbed alongside the `Blob` itself, to their left, right, or split evenly to both sides):
/// - `3p`: `0` other players (`SelfOnly`)
/// - `4p`: `1` other player (`Left1`, `Right1`)
/// - `5–6p`: `2` other players (`Left2`, `Right2`, `LeftAndRight`)
/// - `7–8p`: `3` other players (`Left3`, `Right3`)
/// - `9–10p`: `4` other players (`Left4`, `Right4`, `Left2Right2`)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlobShape {
    SelfOnly,
    Left1,
    Right1,
    LeftAndRight,
    Left2,
    Right2,
    Left3,
    Right3,
    Left4,
    Right4,
    Left2Right2,
}

impl BlobShape {
    pub(crate) fn roll(player_count: usize) -> Self {
        let pool: &[Self] = match player_count {
            ..=3 => &[Self::SelfOnly],
            4 => &[Self::Left1, Self::Right1],
            5..=6 => &[Self::Left2, Self::Right2, Self::LeftAndRight],
            7..=8 => &[Self::Left3, Self::Right3],
            _ => &[Self::Left4, Self::Right4, Self::Left2Right2],
        };
        pool[fastrand::usize(..pool.len())]
    }

    fn default_for_count(player_count: usize) -> Self {
        match player_count {
            ..=3 => Self::SelfOnly,
            4 => Self::Left1,
            5..=6 => Self::LeftAndRight,
            7..=8 => Self::Left3,
            _ => Self::Left2Right2,
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::SelfOnly => "0 other players (only the Blob itself)",
            Self::Left1 => "1 player to their LEFT",
            Self::Right1 => "1 player to their RIGHT",
            Self::LeftAndRight => "both immediate LEFT & RIGHT neighbors",
            Self::Left2 => "2 players to their LEFT",
            Self::Right2 => "2 players to their RIGHT",
            Self::Left3 => "3 players to their LEFT",
            Self::Right3 => "3 players to their RIGHT",
            Self::Left4 => "4 players to their LEFT",
            Self::Right4 => "4 players to their RIGHT",
            Self::Left2Right2 => "2 players to their LEFT & 2 players to their RIGHT",
        }
    }

    pub(crate) fn absorbed_seats(self, n: usize, seat: usize) -> SeatSet {
        let step_left = |k: usize| (seat + n - (k % n)) % n;
        let step_right = |k: usize| (seat + k) % n;
        let mut set = SeatSet::single(seat);
        match self {
            Self::SelfOnly => {}
            Self::Left1 => {
                set.insert(step_left(1));
            }
            Self::Right1 => {
                set.insert(step_right(1));
            }
            Self::LeftAndRight => {
                set.insert(step_left(1));
                set.insert(step_right(1));
            }
            Self::Left2 => {
                for k in 1..=2 {
                    set.insert(step_left(k));
                }
            }
            Self::Right2 => {
                for k in 1..=2 {
                    set.insert(step_right(k));
                }
            }
            Self::Left3 => {
                for k in 1..=3 {
                    set.insert(step_left(k));
                }
            }
            Self::Right3 => {
                for k in 1..=3 {
                    set.insert(step_right(k));
                }
            }
            Self::Left4 => {
                for k in 1..=4 {
                    set.insert(step_left(k));
                }
            }
            Self::Right4 => {
                for k in 1..=4 {
                    set.insert(step_right(k));
                }
            }
            Self::Left2Right2 => {
                for k in 1..=2 {
                    set.insert(step_left(k));
                    set.insert(step_right(k));
                }
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
        let n = ctx.roster.len();
        let shape = ctx
            .board
            .get_key::<ActiveBlobShape>()
            .0
            .unwrap_or_else(|| BlobShape::default_for_count(n));
        let absorbed = shape.absorbed_seats(n, seat);
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
        let n = rctx.roster.len();
        let shape = board
            .get_key::<ActiveBlobShape>()
            .0
            .unwrap_or_else(|| BlobShape::default_for_count(n));
        let absorbed = shape.absorbed_seats(n, seat);
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
            .unwrap_or_else(|| BlobShape::default_for_count(rctx.roster.len()));
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
