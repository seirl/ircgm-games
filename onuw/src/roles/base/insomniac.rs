use ircgm::prelude::*;

use crate::board::BoardMut;
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, Team, WakeStep};

pub struct Insomniac;
pub static INSOMNIAC: Insomniac = Insomniac;

impl Role for Insomniac {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "insomniac",
            "Insomniac",
            GameSet::Base,
            Team::Village,
            "Wake at the very end of the night to check your own card and see if it was swapped.",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WakeStep::passive(&INSOMNIAC, Self::inspect)] }
    }
}

impl Insomniac {
    fn inspect(seat: usize, actor_label: &str, board: &mut BoardMut<'_>, ctx: &mut ResolveCtx<'_>) {
        let Some(seen) = board.peek_player(seat, seat) else {
            ctx.dm(
                seat,
                format!(
                    "🦉 {actor_label}: Your card has a Sentinel Shield on it—it could not have been moved!"
                ),
            );
            return;
        };
        ctx.dm(
            seat,
            irc_format!(
                "🦉 {actor_label}: You check your card before dawn — it is {}!",
                seen.display_for_peek()
            ),
        );
        ctx.record(irc_format!(
            "🦉 {actor_label} ({}) checked their card and saw {}.",
            ctx.nick(seat).bold(),
            seen.display_for_peek(),
        ));
    }
}

#[cfg(test)]
#[path = "insomniac_test.rs"]
mod test;
