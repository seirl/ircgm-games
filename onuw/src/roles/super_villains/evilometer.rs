use ircgm::prelude::*;

use crate::board::BoardMut;
use crate::marks::Mark;
use crate::role::{GameSet, NightFaction, ResolveCtx, Role, RoleInfo, Team, WakeStep};

pub struct Evilometer;
pub static EVILOMETER: Evilometer = Evilometer;

impl Role for Evilometer {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "evilometer",
            "Evilometer",
            GameSet::SuperVillains,
            Team::Village,
            "Wake during the Villain phase: your gadget beeps if either immediate neighbor is a waking Super Villain (and Villains see who holds Evilometer)!",
        )
        .aliases(&["meter", "evil_meter"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WakeStep::passive(&EVILOMETER, 125, Self::inspect).eyes_closed()] }
    }
}

impl Evilometer {
    fn inspect(seat: usize, actor_label: &str, board: &mut BoardMut<'_>, ctx: &mut ResolveCtx<'_>) {
        let (left, right) = ctx.roster.neighbors(seat);
        let neighbor_villain = [left, right].into_iter().any(|n| {
            ctx.waking_role(n).night_faction() == Some(NightFaction::SuperVillain)
                && board.mark_at(n) != Mark::Fear
        });

        let actor_nick = ctx.nick(seat);
        if neighbor_villain {
            ctx.dm(
                seat,
                irc_format!(
                    "{} A Super Villain sits immediately to your left or right!",
                    "📟🚨 *BEEP! BEEP!*".bold().red(),
                ),
            );
            ctx.record(irc_format!(
                "📟 {actor_label} ({}) beeped — a Super Villain neighbor was detected!",
                actor_nick.bold(),
            ));
        } else {
            ctx.dm(
                seat,
                "📟 Evilometer stayed silent (neither immediate neighbor is a waking Super Villain).",
            );
            ctx.record(irc_format!(
                "📟 {actor_label} ({}) stayed silent (no Super Villain neighbors).",
                actor_nick.bold(),
            ));
        }
    }
}
