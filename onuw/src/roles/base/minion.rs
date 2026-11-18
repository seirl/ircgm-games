use ircgm::prelude::*;

use crate::board::BoardMut;
use crate::role::{GameSet, NightFaction, ResolveCtx, Role, RoleInfo, Team, WakeStep};

pub struct Minion;
pub static MINION: Minion = Minion;

impl Role for Minion {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "minion",
            "Minion",
            GameSet::Base,
            Team::Werewolf,
            "Wake to learn who the Werewolves are (they do not know who you are). Win if Werewolves survive!",
        )
        .minion()
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WakeStep::passive(&MINION, Self::inspect).copy_immediately()] }
    }
}

impl Minion {
    fn inspect(
        seat: usize,
        actor_label: &str,
        _board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let has_tanner = ctx.roster.deck_has(&super::tanner::TANNER);
        let wolves = ctx.seats_with_faction(NightFaction::Werewolf);
        let actor_nick = ctx.nick(seat);
        if wolves.is_empty() {
            let goal = if has_tanner {
                "Survive and get another non-Tanner player killed to win!"
            } else {
                "Survive and get another player killed to win!"
            };
            ctx.dm(
                seat,
                irc_format!(
                    "🗡️ {actor_label}: {}! {goal}",
                    "NO players hold Werewolf cards".bold().yellow()
                ),
            );
            ctx.record(irc_format!(
                "🗡️ {actor_label} ({}) saw no Werewolves in play.",
                actor_nick.bold(),
            ));
        } else {
            let wolf_names = ctx.roster.format_seats(wolves);
            ctx.dm(
                seat,
                irc_format!(
                    "🗡️ {actor_label}: The Werewolves are {} (they do not know who you are!).",
                    wolf_names.as_str().bold().red()
                ),
            );
            ctx.record(irc_format!(
                "🗡️ {actor_label} ({}) saw Werewolf pack: {}.",
                actor_nick.bold(),
                wolf_names.bold().red(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "minion_test.rs"]
mod test;
