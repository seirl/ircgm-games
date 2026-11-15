use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, GroupEffect, GroupPromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, Team,
    Wake,
};

pub struct Henchman;
pub static HENCHMAN: Henchman = Henchman;

enum VillainIntent {
    Syndicate,
    LonePeek(Answer<Option<usize>>),
}

impl Role for Henchman {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "henchman",
            "Henchman",
            GameSet::SuperVillains,
            Team::Werewolf,
            "Wake with the Super Villains; if you are the sole Villain in play, you may peek at 1 center card!",
        )
        .tags(&[RoleTag::VillainPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::pack(121, RoleTag::VillainPack, Self::huddle))
    }
}

impl Henchman {
    fn huddle<'a>(
        active_seats: Vec<usize>,
        ctx: GroupPromptCtx<'a>,
    ) -> BoxFuture<'a, Result<GroupEffect>> {
        Box::pin(async move {
            let intent = if active_seats.len() >= 2 {
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
                VillainIntent::Syndicate
            } else {
                let choice = ctx
                    .ask(
                        active_seats[0],
                        irc_format!(
                            "🦹‍♂️ You are the {}! Pick ONE center card to peek ({}, {}, {}, or {}):",
                            "SOLO SUPER VILLAIN".bold().red(),
                            "1".bold(),
                            "2".bold(),
                            "3".bold(),
                            "pass".bold(),
                        ),
                        |m| {
                            if Roster::is_pass(&m.text) {
                                return AskMatch::accept(None);
                            }
                            match Roster::parse_center_slot(&m.text) {
                                Some(c_idx) => AskMatch::accept(Some(c_idx)),
                                None => AskMatch::reject("Reply 1, 2, 3, or pass.".yellow()),
                            }
                        },
                    )
                    .await?;
                VillainIntent::LonePeek(choice)
            };

            Ok(GroupEffect::new(move |board, rctx| {
                Self::resolve(&active_seats, intent, board, rctx);
            }))
        })
    }

    fn resolve(
        active_seats: &[usize],
        intent: VillainIntent,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        match intent {
            VillainIntent::Syndicate => {
                let names = ctx.roster.format_indices(active_seats);
                ctx.record(irc_format!(
                    "🦹‍♂️ Super Villains ({}) assembled in the Evil Lair.",
                    names.bold().red(),
                ));
            }
            VillainIntent::LonePeek(Answer::Ok(Some(c_idx))) => {
                let seat = active_seats[0];
                board.record_card_interaction(seat);
                let seen = board.center[c_idx];
                ctx.dm(
                    seat,
                    irc_format!("🦹‍♂️ Center #{} is: {}", c_idx + 1, seen.display_for_peek()),
                );
                ctx.record(irc_format!(
                    "🦹‍♂️ Solo Villain {} peeked Center #{} ({}).",
                    ctx.nick(seat).bold(),
                    c_idx + 1,
                    seen.display_for_peek(),
                ));
            }
            VillainIntent::LonePeek(_) => {
                let seat = active_seats[0];
                ctx.record(irc_format!(
                    "🦹‍♂️ Solo Villain {} did not peek the center.",
                    ctx.nick(seat).bold(),
                ));
            }
        }
    }
}
