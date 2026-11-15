use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, GroupEffect, GroupPromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, Team,
    Wake,
};

pub struct Werewolf;
pub static WEREWOLF: Werewolf = Werewolf;

enum WolfIntent {
    Pack,
    LonePeek(Answer<Option<usize>>),
}

impl Role for Werewolf {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "werewolf",
            "Werewolf",
            GameSet::Base,
            Team::Werewolf,
            "See your fellow Werewolves; if you are the Lone Wolf, you may peek at 1 center card.",
        )
        .aliases(&["ww", "wolf"])
        .tags(&[RoleTag::WolfPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::pack(120, RoleTag::WolfPack, Self::huddle))
    }
}

impl Werewolf {
    fn huddle<'a>(
        active_seats: Vec<usize>,
        ctx: GroupPromptCtx<'a>,
    ) -> BoxFuture<'a, Result<GroupEffect>> {
        Box::pin(async move {
            let intent = if active_seats.len() >= 2 {
                for &w_seat in &active_seats {
                    let partners: Vec<usize> = active_seats
                        .iter()
                        .copied()
                        .filter(|&o| o != w_seat)
                        .collect();
                    ctx.dm(
                        w_seat,
                        irc_format!(
                            "🐺 Fellow Werewolf pack: {} (chat here to coordinate!)",
                            ctx.roster.format_indices(&partners).bold().red()
                        ),
                    );
                }
                ctx.relay_huddle("Pack".red().bold(), &active_seats).await?;
                WolfIntent::Pack
            } else {
                let choice = ctx
                    .ask(
                        active_seats[0],
                        irc_format!(
                            "🐺 You are the {}! Pick ONE center card to peek (`1`, `2`, `3`, or `pass`):",
                            "LONE WOLF".bold().red()
                        ),
                        |m| {
                            if Roster::is_pass(&m.text) {
                                return AskMatch::accept(None);
                            }
                            match Roster::parse_center_slot(&m.text) {
                                Some(c_idx) => AskMatch::accept(Some(c_idx)),
                                None => AskMatch::reject("Reply `1`, `2`, `3`, or `pass`.".yellow()),
                            }
                        },
                    )
                    .await?;
                WolfIntent::LonePeek(choice)
            };

            Ok(GroupEffect::new(move |board, rctx| {
                Self::resolve(&active_seats, intent, board, rctx);
            }))
        })
    }

    fn resolve(
        active_seats: &[usize],
        intent: WolfIntent,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        match intent {
            WolfIntent::Pack => {
                let names = ctx.roster.format_indices(active_seats);
                ctx.record(irc_format!(
                    "🐺 Werewolves ({}) recognized each other.",
                    names.bold().red(),
                ));
            }
            WolfIntent::LonePeek(Answer::Ok(Some(c_idx))) => {
                let seat = active_seats[0];
                board.record_card_interaction(seat);
                let seen = board.center[c_idx];
                ctx.dm(
                    seat,
                    irc_format!("🐺 Center #{} is: {}", c_idx + 1, seen.display_for_peek()),
                );
                ctx.record(irc_format!(
                    "🐺 Lone Wolf {} peeked Center #{} ({}).",
                    ctx.nick(seat).bold().red(),
                    c_idx + 1,
                    seen.display_for_peek(),
                ));
            }
            WolfIntent::LonePeek(_) => {
                let seat = active_seats[0];
                ctx.record(irc_format!(
                    "🐺 Lone Wolf {} passed on peeking the center.",
                    ctx.nick(seat).bold().red(),
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "werewolf_test.rs"]
mod test;
