use ircgm::prelude::*;

use super::evilometer::EVILOMETER;
use crate::board::BoardCards;
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, Team, Wake,
};

pub struct Henchman;
pub static HENCHMAN: Henchman = Henchman;

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
        .tags(&[RoleTag::VillainPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::pack(124, RoleTag::VillainPack, |seats, ctx| {
            Box::pin(Self::huddle(seats, ctx))
        }))
    }
}

impl Henchman {
    async fn huddle(active_seats: Vec<usize>, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let evilometers = ctx.seats_with_role(&EVILOMETER);
        let evilometer_list = ctx.roster.format_indices(&evilometers);

        for &v_seat in &active_seats {
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

        if active_seats.len() >= 2 {
            for &v_seat in &active_seats {
                let partners: Vec<usize> = active_seats
                    .iter()
                    .copied()
                    .filter(|&o| o != v_seat)
                    .collect();
                ctx.dm(
                    v_seat,
                    irc_format!(
                        "🦹‍♂️ Evil Syndicate: Fellow Super Villains: {} (chat here to coordinate!)",
                        ctx.roster.format_indices(&partners).bold().red(),
                    ),
                );
            }
            ctx.relay_huddle("Syndicate".red().bold(), &active_seats)
                .await?;
        } else {
            ctx.dm(
                active_seats[0],
                "🦹‍♂️ Evil Syndicate: You are the sole waking Super Villain tonight!",
            );
        }

        Ok(GroupEffect::new(move |board, rctx| {
            Self::resolve(&active_seats, board, rctx);
        }))
    }

    fn resolve(active_seats: &[usize], _board: &mut BoardCards, ctx: &mut ResolveCtx<'_>) {
        if active_seats.len() >= 2 {
            let names = ctx.roster.format_indices(active_seats);
            ctx.record(irc_format!(
                "🦹‍♂️ Super Villains ({}) assembled in the Evil Lair.",
                names.bold().red(),
            ));
        } else {
            let seat = active_seats[0];
            ctx.record(irc_format!(
                "🦹‍♂️ Solo Super Villain {} woke in the Evil Lair.",
                ctx.nick(seat).bold(),
            ));
        }
    }
}
