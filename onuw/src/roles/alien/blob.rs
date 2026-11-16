use ircgm::prelude::*;

use crate::board::{BoardMut, Roster, SeatSet};
use crate::role::{
    GameSet, ResolveCtx, Role, RoleInfo, SpecialVictory, Team, VictoryCtx, WakeStep,
};

pub struct Blob;
pub static BLOB: Blob = Blob;

fn blob_seats_around(roster: &Roster, seat: usize) -> SeatSet {
    let (left, right) = roster.neighbors(seat);
    let mut set = SeatSet::from([seat, left]);
    if roster.len() >= 6 {
        set.insert(right);
    }
    set
}

impl Role for Blob {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "blob",
            "Blob",
            GameSet::Alien,
            Team::Independent,
            "Part of the Blob! You win ONLY if you and every neighbor absorbed into the Blob survive the vote!",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WakeStep::passive(&BLOB, 196, Self::inspect)] }
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        let absorbed = blob_seats_around(ctx.roster, seat);
        absorbed
            .is_disjoint(ctx.dead_seats)
            .then_some(SpecialVictory::independent())
    }
}

impl Blob {
    fn inspect(seat: usize, actor_label: &str, _board: &mut BoardMut<'_>, ctx: &mut ResolveCtx<'_>) {
        let absorbed = blob_seats_around(ctx.roster, seat);
        let names = ctx.roster.format_seats(absorbed);
        ctx.dm(
            seat,
            irc_format!(
                "🟢 Blob: At your seat, the Blob spans {} — keep ALL absorbed players alive to win!",
                names.as_str().bold().green(),
            ),
        );
        ctx.record(irc_format!(
            "🟢 {actor_label} ({}) woke with Blob neighbors {}.",
            ctx.nick(seat).bold(),
            names.as_str().bold().green(),
        ));
    }
}
