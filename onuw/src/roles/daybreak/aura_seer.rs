use ircgm::prelude::*;

use crate::board::BoardCards;
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, Team, Wake};

pub struct AuraSeer;
pub static AURA_SEER: AuraSeer = AuraSeer;

impl Role for AuraSeer {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "aura_seer",
            "Aura Seer",
            GameSet::Daybreak,
            Team::Village,
            "Wake at the end of Night to see every player who viewed or moved a card tonight!",
        )
        .aliases(&["aura"])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::passive(195, Self::inspect))
    }
}

impl AuraSeer {
    fn inspect(
        seat: usize,
        actor_label: &str,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let mut active: Vec<usize> = board
            .viewed_or_moved_actors
            .iter()
            .copied()
            .filter(|&s| s != seat)
            .collect();
        active.sort_unstable();

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
            let names_str = ctx.roster.format_indices(&active);
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
