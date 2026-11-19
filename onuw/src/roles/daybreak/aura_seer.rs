use ircgm::prelude::*;

use crate::board::BoardMut;
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, Team, WakeStep};

pub struct AuraSeer;
pub static AURA_SEER: AuraSeer = AuraSeer;

impl Role for AuraSeer {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "aura_seer",
            "Aura Seer",
            GameSet::Daybreak,
            Team::Village,
            "Wake after Troublemaker & Village Idiot to see every player who viewed or moved a card so far!",
        )
        .aliases(&["aura"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WakeStep::passive(&AURA_SEER, Self::inspect)] }
    }
}

impl AuraSeer {
    fn inspect(seat: usize, actor_label: &str, board: &mut BoardMut<'_>, ctx: &mut ResolveCtx<'_>) {
        // Rule (Bonus Roles BP1 — Aura Seer): "When the Aura Seer wakes up,
        // all players who have (at that point in the night) viewed or moved any
        // card stick out their thumbs." Tracks cards only (not Marks, Shields,
        // or taps).
        let active = board.aura_actors_excluding(seat);

        let actor_nick = ctx.nick(seat);
        if active.is_empty() {
            ctx.dm(
                seat,
                "✨ Aura Seer: Nobody else viewed or moved any cards tonight!",
            );
            ctx.record(irc_format!(
                "✨ {actor_label} ({}) sensed no active auras.",
                actor_nick.bold(),
            ));
        } else {
            let names_str = ctx.roster.format_seats(active);
            ctx.dm(
                seat,
                irc_format!(
                    "✨ Aura Seer: Players who viewed or moved a card tonight: {}",
                    names_str.as_str().bold().light_cyan(),
                ),
            );
            ctx.record(irc_format!(
                "✨ {actor_label} ({}) sensed active auras on: {}.",
                actor_nick.bold(),
                names_str.as_str().bold().light_cyan(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "aura_seer_test.rs"]
mod test;
