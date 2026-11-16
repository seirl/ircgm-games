use ircgm::prelude::*;

use super::evilometer::EVILOMETER;
use crate::board::{BoardMut, SeatSet};
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, NightFaction, ResolveCtx, Role, RoleInfo, Team, WakeStep,
};

pub struct Henchman;
pub static HENCHMAN: Henchman = Henchman;

pub(crate) const VILLAIN_PACK_STEP: WakeStep = WakeStep::pack(
    NightFaction::SuperVillain,
    124,
    |seats, ctx| Box::pin(Henchman::huddle(seats, ctx)),
);

impl Role for Henchman {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "henchman",
            "Henchman #7",
            GameSet::SuperVillains,
            Team::SuperVillain,
            "Wake with the Super Villains to recognize fellow Super Villains and spot the Evilometer!",
        )
        .aliases(&["henchman", "henchman7", "henchman #7"])
        .night_faction(NightFaction::SuperVillain)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[VILLAIN_PACK_STEP] }
    }
}

impl Henchman {
    async fn huddle(active_seats: SeatSet, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let evilometers = ctx.seats_with_role(&EVILOMETER);
        let evilometer_list = ctx.roster.format_seats(evilometers);

        for v_seat in active_seats {
            if !evilometers.is_empty() {
                ctx.dm(
                    v_seat,
                    irc_format!(
                        "📟🦹‍♂️ Syndicate Radar: {} holds the Evilometer!",
                        evilometer_list.as_str().bold().yellow(),
                    ),
                );
            }
        }

        if let Some(solo) = active_seats.only() {
            ctx.dm(
                solo,
                "🦹‍♂️ Evil Syndicate: You are the sole waking Super Villain tonight!",
            );
        } else {
            for v_seat in active_seats {
                let partners = active_seats.without(v_seat);
                ctx.dm(
                    v_seat,
                    irc_format!(
                        "🦹‍♂️ Evil Syndicate: Fellow Super Villains: {} (chat here to coordinate!)",
                        ctx.roster.format_seats(partners).bold().red(),
                    ),
                );
            }
            ctx.relay_huddle("Syndicate".red().bold(), active_seats)
                .await?;
        }

        Ok(GroupEffect::new(move |board, rctx| {
            Self::resolve(active_seats, board, rctx);
        }))
    }

    fn resolve(active_seats: SeatSet, _board: &mut BoardMut<'_>, ctx: &mut ResolveCtx<'_>) {
        if let Some(solo) = active_seats.only() {
            ctx.record(irc_format!(
                "🦹‍♂️ Solo Super Villain {} woke in the Evil Lair.",
                ctx.nick(solo).bold(),
            ));
        } else {
            let names = ctx.roster.format_seats(active_seats);
            ctx.record(irc_format!(
                "🦹‍♂️ Super Villains ({}) assembled in the Evil Lair.",
                names.bold().red(),
            ));
        }
    }
}
