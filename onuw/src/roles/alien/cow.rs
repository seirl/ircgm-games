use ircgm::prelude::*;

use crate::board::BoardCards;
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, RoleTag, Team, Wake};

pub struct Cow;
pub static COW: Cow = Cow;

impl Role for Cow {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "cow",
            "Cow",
            GameSet::Alien,
            Team::Village,
            "Stick out your hoof: you feel a tap if your immediate left OR right neighbor is an Alien!",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::passive(113, Self::inspect))
    }
}

impl Cow {
    fn inspect(
        seat: usize,
        actor_label: &str,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let (left, right) = ctx.roster.neighbors(seat);
        let neighbor_alien = [left, right].into_iter().any(|n| {
            board.players[n]
                .effective_role()
                .tags()
                .contains(&RoleTag::AlienPack)
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
                "🐄 Nobody tapped your hoof tonight (neither neighbor is an Alien).",
            );
            ctx.record(irc_format!(
                "🐄 {actor_label} ({}) felt no hoof tap.",
                actor_nick.bold(),
            ));
        }
    }
}
