use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardKey, BoardMut, Roster, SeatSet};
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, SpecialVictory, Team, VictoryCtx, WakeStep};

/// Per-game randomized absorption shape announced publicly at `#13 Blob`.
///
/// Official *One Night Ultimate Alien* player-count absorption table (contiguous adjacent players
/// absorbed alongside the `Blob` itself, to their left, right, or split evenly to both sides):
/// - `3p`: `0` other players (`0, 0`)
/// - `4p`: `1` other player (`1, 0` or `0, 1`)
/// - `5–6p`: `2` other players (`2, 0`, `0, 2`, or `1, 1`)
/// - `7–8p`: `3` other players (`3, 0` or `0, 3`)
/// - `9–10p`: `4` other players (`4, 0`, `0, 4`, or `2, 2`)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BlobShape {
    pub left: u8,
    pub right: u8,
}

impl BlobShape {
    pub(crate) const SELF_ONLY: Self = Self::new(0, 0);
    pub(crate) const LEFT_1: Self = Self::new(1, 0);
    pub(crate) const RIGHT_1: Self = Self::new(0, 1);
    pub(crate) const LEFT_AND_RIGHT: Self = Self::new(1, 1);
    pub(crate) const LEFT_2: Self = Self::new(2, 0);
    pub(crate) const RIGHT_2: Self = Self::new(0, 2);
    pub(crate) const LEFT_3: Self = Self::new(3, 0);
    pub(crate) const RIGHT_3: Self = Self::new(0, 3);
    pub(crate) const LEFT_4: Self = Self::new(4, 0);
    pub(crate) const RIGHT_4: Self = Self::new(0, 4);
    pub(crate) const LEFT_2_RIGHT_2: Self = Self::new(2, 2);

    pub(crate) const fn new(left: u8, right: u8) -> Self {
        Self { left, right }
    }

    pub(crate) fn roll(player_count: usize) -> Self {
        let pool: &[Self] = match player_count {
            ..=3 => &[Self::SELF_ONLY],
            4 => &[Self::LEFT_1, Self::RIGHT_1],
            5..=6 => &[Self::LEFT_2, Self::RIGHT_2, Self::LEFT_AND_RIGHT],
            7..=8 => &[Self::LEFT_3, Self::RIGHT_3],
            _ => &[Self::LEFT_4, Self::RIGHT_4, Self::LEFT_2_RIGHT_2],
        };
        pool[fastrand::usize(..pool.len())]
    }

    const fn default_for_count(player_count: usize) -> Self {
        match player_count {
            ..=3 => Self::SELF_ONLY,
            4 => Self::LEFT_1,
            5..=6 => Self::LEFT_AND_RIGHT,
            7..=8 => Self::LEFT_3,
            _ => Self::LEFT_2_RIGHT_2,
        }
    }

    fn description(self) -> String {
        let unit = |k: u8| if k == 1 { "player" } else { "players" };
        match (self.left, self.right) {
            (0, 0) => "0 other players (only the Blob itself)".to_owned(),
            (1, 1) => "both immediate LEFT & RIGHT neighbors".to_owned(),
            (l, 0) => format!("{l} {} to their LEFT", unit(l)),
            (0, r) => format!("{r} {} to their RIGHT", unit(r)),
            (l, r) => format!("{l} {} to their LEFT & {r} {} to their RIGHT", unit(l), unit(r)),
        }
    }

    pub(crate) fn absorbed_seats(self, n: usize, seat: usize) -> SeatSet {
        let mut set = SeatSet::single(seat);
        for k in 1..=usize::from(self.left) {
            set.insert((seat + n - (k % n)) % n);
        }
        for k in 1..=usize::from(self.right) {
            set.insert((seat + k) % n);
        }
        set
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ActiveBlobShape(pub Option<BlobShape>);

impl BoardKey for ActiveBlobShape {
    const ACCESS: u16 = BoardAccess::ROLE_STATE;

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
        let desc = board
            .get_key::<ActiveBlobShape>()
            .0
            .unwrap_or_else(|| BlobShape::default_for_count(rctx.roster.len()))
            .description();
        rctx.announce(irc_format!(
            "🟢 {} Whoever holds the {} is merged with {} ({}) — keep ALL absorbed players alive!",
            "Blob Absorption:".bold().green(),
            BLOB.info().styled_name(),
            desc.as_str().bold().yellow(),
            rctx.roster.format_table_circle(),
        ));
        rctx.record(irc_format!(
            "🟢 Blob absorption rolled: {}.",
            desc.as_str().bold().green(),
        ));
    }
}

#[cfg(test)]
#[path = "blob_test.rs"]
mod test;
