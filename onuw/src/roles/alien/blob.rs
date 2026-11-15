use ircgm::prelude::*;

use crate::board::BoardCards;
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, SpecialVictory, Team, VictoryCtx};

pub struct Blob;
pub static BLOB: Blob = Blob;

impl Role for Blob {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "blob",
            "Blob",
            GameSet::Alien,
            Team::Independent,
            "Part of the Blob! You win ONLY if every player absorbed into the Blob survives the vote!",
        )
    }

    fn on_deal(&self, seat: usize, board: &mut BoardCards, ctx: &mut ResolveCtx<'_>) {
        let (left, right) = ctx.roster.neighbors(seat);
        board.blob_seats.insert(seat);
        board.blob_seats.insert(left);
        if ctx.roster.len() >= 6 {
            board.blob_seats.insert(right);
        }

        let mut list: Vec<usize> = board.blob_seats.iter().copied().collect();
        list.sort_unstable();
        let names = ctx.roster.format_indices(&list);
        ctx.dm(
            seat,
            irc_format!(
                "🟢 Blob: You have absorbed {} — keep ALL of them alive to win!",
                names.as_str().bold().green(),
            ),
        );
        ctx.record(irc_format!(
            "🟢 {} ({}) absorbed {} into the Blob.",
            self.info().styled_name(),
            ctx.nick(seat).bold(),
            names.as_str().bold().green(),
        ));
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        let won = if ctx.board.blob_seats.is_empty() {
            !ctx.is_dead(seat)
        } else {
            ctx.board.blob_seats.iter().all(|&s| !ctx.is_dead(s))
        };
        won.then_some(SpecialVictory::independent())
    }
}
