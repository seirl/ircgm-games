use ircgm::prelude::*;

use crate::board::BoardMut;
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, Team, WakeStep};

pub struct SelfAwarenessGirl;
pub static SELF_AWARENESS_GIRL: SelfAwarenessGirl = SelfAwarenessGirl;

impl Role for SelfAwarenessGirl {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "self_awareness_girl",
            "Self-Awareness Girl",
            GameSet::SuperVillains,
            Team::Village,
            "Wake at the very end of the night to check your own card and see if it was swapped.",
        )
        .aliases(&["selfawareness", "sag"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WakeStep::passive(&SELF_AWARENESS_GIRL, Self::inspect)] }
    }
}

impl SelfAwarenessGirl {
    fn inspect(
        seat: usize,
        actor_label: &str,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let Some(seen) = board.peek_player(seat, seat) else {
            ctx.dm(
                seat,
                format!(
                    "🦸‍♀️ {actor_label}: Your card has a Sentinel Shield on it — it could not have been moved!"
                ),
            );
            return;
        };
        ctx.dm(
            seat,
            irc_format!(
                "🦸‍♀️ {actor_label}: You check your card before dawn — it is {}!",
                seen.display_for_peek(),
            ),
        );
        ctx.record(irc_format!(
            "🦸‍♀️ {actor_label} ({}) checked their card and saw {}.",
            ctx.nick(seat).bold(),
            seen.display_for_peek(),
        ));
    }
}

#[cfg(test)]
#[path = "self_awareness_girl_test.rs"]
mod test;
