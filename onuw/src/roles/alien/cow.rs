use ircgm::prelude::*;

use crate::board::BoardMut;
use crate::marks::Mark;
use crate::role::{GameSet, NightFaction, ResolveCtx, Role, RoleInfo, Team, WakeStep};

pub struct Cow;
pub static COW: Cow = Cow;

impl Role for Cow {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "cow",
            "Cow",
            GameSet::Alien,
            Team::Village,
            "Stick out your hoof: you feel a tap if your immediate left OR right neighbor is a waking Alien!",
        )
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WakeStep::passive(&COW, 113, Self::inspect).eyes_closed()] }
    }
}

impl Cow {
    fn inspect(seat: usize, actor_label: &str, board: &mut BoardMut<'_>, ctx: &mut ResolveCtx<'_>) {
        let (left, right) = ctx.roster.neighbors(seat);
        let neighbor_alien = [left, right].into_iter().any(|n| {
            ctx.waking_role(n).night_faction() == Some(NightFaction::Alien)
                && board.mark_at(n) != Mark::Fear
        });

        let actor_nick = ctx.nick(seat);
        if neighbor_alien {
            ctx.dm(
                seat,
                irc_format!(
                    "{} An Alien sits immediately to your left or right! Moo!",
                    "🐄🛸 *TAP!*".bold().yellow()
                ),
            );
            ctx.record(irc_format!(
                "🐄 {actor_label} ({}) felt an Alien tap their hoof!",
                actor_nick.bold(),
            ));
        } else {
            ctx.dm(
                seat,
                "🐄 Nobody tapped your hoof tonight (neither neighbor is a waking Alien).",
            );
            ctx.record(irc_format!(
                "🐄 {actor_label} ({}) felt no hoof tap.",
                actor_nick.bold(),
            ));
        }
    }
}
