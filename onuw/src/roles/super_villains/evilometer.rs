use ircgm::prelude::*;

use crate::board::BoardCards;
use crate::role::{GameSet, ResolveCtx, Role, RoleInfo, RoleTag, Team, Wake};

pub struct Evilometer;
pub static EVILOMETER: Evilometer = Evilometer;

impl Role for Evilometer {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "evilometer",
            "Evilometer",
            GameSet::SuperVillains,
            Team::Village,
            "Wake at the end of Night: your gadget beeps if either immediate neighbor holds a Super Villain or Evil threat card!",
        )
        .aliases(&["meter", "evil_meter"])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::passive(196, Self::inspect))
    }
}

impl Evilometer {
    fn inspect(
        seat: usize,
        actor_label: &str,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let (left, right) = ctx.roster.neighbors(seat);
        let neighbor_evil = [left, right].into_iter().any(|n| {
            let tags = board.players[n].effective_role().tags();
            tags.contains(&RoleTag::VillainPack) || board.is_primary_threat(n)
        });

        let actor_nick = ctx.nick(seat);
        if neighbor_evil {
            ctx.dm(
                seat,
                irc_format!(
                    "{} Evil detected immediately to your left or right!",
                    "📟🚨 *BEEP! BEEP!*".bold().red(),
                ),
            );
            ctx.record(irc_format!(
                "📟 {actor_label} ({}) beeped — an Evil neighbor was detected!",
                actor_nick.bold(),
            ));
        } else {
            ctx.dm(
                seat,
                "📟 Evilometer stayed silent (neither immediate neighbor is Evil).",
            );
            ctx.record(irc_format!(
                "📟 {actor_label} ({}) stayed silent (no Evil neighbors).",
                actor_nick.bold(),
            ));
        }
    }
}
