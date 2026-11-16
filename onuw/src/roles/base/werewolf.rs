use ircgm::prelude::*;

use crate::board::{BoardMut, Roster};
use crate::role::{
    GameSet, GroupEffect, GroupPromptCtx, NightFaction, ResolveCtx, Role, RoleInfo, Team, WakeStep,
};

pub struct Werewolf;
pub static WEREWOLF: Werewolf = Werewolf;

pub(crate) const WOLF_PACK_STEP: WakeStep = WakeStep::pack(
    NightFaction::Werewolf,
    120,
    |seats, ctx| Box::pin(Werewolf::huddle(seats, ctx)),
);

enum WolfIntent {
    Pack { dream_wolves: Vec<usize> },
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
        .night_faction(NightFaction::Werewolf)
    }

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WOLF_PACK_STEP] }
    }
}

impl Werewolf {
    async fn huddle(active_seats: Vec<usize>, ctx: GroupPromptCtx<'_>) -> Result<GroupEffect> {
        let dream_wolves: Vec<usize> = ctx
            .waking_seats_with_faction(NightFaction::Werewolf)
            .into_iter()
            .filter(|s| !active_seats.contains(s))
            .collect();

        let intent = if active_seats.len() >= 2 {
            for &w_seat in &active_seats {
                let partners: Vec<usize> = active_seats
                    .iter()
                    .copied()
                    .filter(|&o| o != w_seat)
                    .collect();
                if dream_wolves.is_empty() {
                    ctx.dm(
                        w_seat,
                        irc_format!(
                            "🐺 Fellow Werewolf pack: {} (chat here to coordinate!)",
                            ctx.roster.format_indices(&partners).bold().red()
                        ),
                    );
                } else {
                    ctx.dm(
                        w_seat,
                        irc_format!(
                            "🐺 Fellow Werewolf pack: {} · 💤 Dream Wolf (asleep): {} (chat here to coordinate!)",
                            ctx.roster.format_indices(&partners).bold().red(),
                            ctx.roster.format_indices(&dream_wolves).bold().red(),
                        ),
                    );
                }
            }
            ctx.relay_huddle("Pack".red().bold(), &active_seats).await?;
            WolfIntent::Pack { dream_wolves }
        } else if !dream_wolves.is_empty() {
            ctx.dm(
                active_seats[0],
                irc_format!(
                    "🐺 Fellow Werewolf pack: 💤 Dream Wolf {} (asleep — you are not the Lone Wolf!).",
                    ctx.roster.format_indices(&dream_wolves).bold().red(),
                ),
            );
            WolfIntent::Pack { dream_wolves }
        } else {
            let choice = ctx
                .ask(
                    active_seats[0],
                    irc_format!(
                        "🐺 You are the {}! Pick ONE center card to peek ({}, {}, {}, or {}):",
                        "LONE WOLF".bold().red(),
                        "1".bold(),
                        "2".bold(),
                        "3".bold(),
                        "pass".bold(),
                    ),
                    |m| {
                        if Roster::is_pass(&m.text) {
                            return AskMatch::accept_say(None, "🐺 Passed on peeking the center.");
                        }
                        match Roster::parse_center_slot(&m.text) {
                            Some(c_idx) => AskMatch::accept_say(
                                Some(c_idx),
                                format!(
                                    "🐺 Locked in Center #{} — card reveals when the phase ends!",
                                    c_idx + 1
                                ),
                            ),
                            None => AskMatch::reject("Reply 1, 2, 3, or pass.".yellow()),
                        }
                    },
                )
                .await?;
            WolfIntent::LonePeek(choice)
        };

        Ok(GroupEffect::new(move |board, rctx| {
            Self::resolve(&active_seats, intent, board, rctx);
        }))
    }

    fn resolve(
        active_seats: &[usize],
        intent: WolfIntent,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        match intent {
            WolfIntent::Pack { dream_wolves } => {
                let names = ctx.roster.format_indices(active_seats);
                if dream_wolves.is_empty() {
                    ctx.record(irc_format!(
                        "🐺 Werewolves ({}) recognized each other.",
                        names.bold().red(),
                    ));
                } else {
                    let dw_names = ctx.roster.format_indices(&dream_wolves);
                    ctx.record(irc_format!(
                        "🐺 Werewolves ({}) saw sleeping Dream Wolf ({}).",
                        names.bold().red(),
                        dw_names.bold().red(),
                    ));
                }
            }
            WolfIntent::LonePeek(Answer::Ok(Some(c_idx))) => {
                let seat = active_seats[0];
                let seen = board.peek_center(seat, c_idx);
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
