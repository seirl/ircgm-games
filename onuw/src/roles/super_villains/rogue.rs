//! Super Villains **Rogue** (`Wake::reads_board()` Mid-Night Sequence Point!)
//!
//! Wakes *after* `Robber (160)` and `Troublemaker (170)` have swapped cards:
//! 1. Picks another unshielded player (`<nick>`) to inspect their *post-swap* card.
//! 2. Chooses whether to `steal` it for themselves, `ditch` it into Center #1, or `leave` it!

use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct Rogue;
pub static ROGUE: Rogue = Rogue;

#[derive(Clone, Copy)]
enum RogueDecision {
    Pass,
    Leave { target: usize },
    Steal { target: usize },
    DitchToCenter { target: usize },
}

impl Role for Rogue {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "rogue",
            "Rogue",
            GameSet::SuperVillains,
            Team::Village,
            "Wake AFTER Robber & Troublemaker swaps: inspect 1 player's current card, then `steal` it, `ditch` it into Center #1, or `leave` it!",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(175, Self::prompt).reads_board().mutates())
    }
}

impl Rogue {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let others: Vec<usize> = (0..ctx.roster.len())
                .filter(|&i| i != seat && !ctx.shielded.contains(&i))
                .collect();
            if others.is_empty() {
                ctx.say("🛡️ All other players are shielded tonight!");
                return Ok(SeatEffect::new(move |label, board, rctx| {
                    Self::resolve(seat, label, RogueDecision::Pass, board, rctx);
                }));
            }

            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let step1 = ctx
                .ask(
                    format!(
                        "🗡️ Rogue: Reply `<nick>` to inspect their post-swap card, or `pass`. Players: {}",
                        roster.format_players_marking_shields(shielded)
                    ),
                    move |m| {
                        if Roster::is_pass(&m.text) {
                            return AskMatch::accept(None);
                        }
                        match roster.resolve_nick(&m.text) {
                            Some(idx) if idx == seat => {
                                AskMatch::reject("Pick ANOTHER player, not yourself!".yellow())
                            }
                            Some(idx) if shielded.contains(&idx) => {
                                AskMatch::reject("🛡️ That player is shielded!".yellow())
                            }
                            Some(idx) => AskMatch::accept(Some(idx)),
                            None => AskMatch::reject("Unknown player nickname (or `pass`).".yellow()),
                        }
                    },
                )
                .await?;

            let Answer::Ok(Some(target)) = step1 else {
                return Ok(SeatEffect::new(move |label, board, rctx| {
                    Self::resolve(seat, label, RogueDecision::Pass, board, rctx);
                }));
            };

            let peek_card = ctx.board().players[target];
            let target_nick = ctx.nick(target);
            let prompt2 = irc_format!(
                "🗡️ Rogue: {} currently holds {}! Reply `steal` (take it for yourself), `ditch` (swap it with Center #1), or `leave`:",
                target_nick.bold(),
                peek_card.display_for_peek(),
            );

            let step2 = ctx
                .ask(
                    prompt2,
                    move |m| match m.text.trim().to_ascii_lowercase().as_str() {
                        "steal" | "take" | "rob" => AskMatch::accept_say(
                            RogueDecision::Steal { target },
                            irc_format!(
                                "🗡️ Stole {}'s card!",
                                roster.seats[target].user.nick().bold()
                            ),
                        ),
                        "ditch" | "center" | "yes" | "y" => AskMatch::accept_say(
                            RogueDecision::DitchToCenter { target },
                            irc_format!(
                                "🗡️ Swapped {}'s card with Center #1!",
                                roster.seats[target].user.nick().bold()
                            ),
                        ),
                        "leave" | "no" | "n" | "pass" | "skip" => {
                            AskMatch::accept(RogueDecision::Leave { target })
                        }
                        _ => AskMatch::reject("Reply `steal`, `ditch`, or `leave`.".yellow()),
                    },
                )
                .await?;

            let decision = step2.ok().unwrap_or(RogueDecision::Leave { target });
            Ok(SeatEffect::new(move |label, board, rctx| {
                Self::resolve(seat, label, decision, board, rctx);
            }))
        })
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        decision: RogueDecision,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let actor_nick = ctx.nick(seat);
        match decision {
            RogueDecision::Pass => {
                ctx.record(irc_format!(
                    "🗡️ {actor_label} ({}) stayed in the shadows.",
                    actor_nick.bold(),
                ));
            }
            RogueDecision::Leave { target } => {
                board.record_card_interaction(seat);
                let seen = board.players[target];
                ctx.record(irc_format!(
                    "🗡️ {actor_label} ({}) peeked {} ({}) and left it.",
                    actor_nick.bold(),
                    ctx.nick(target).bold(),
                    seen.display_for_peek(),
                ));
            }
            RogueDecision::Steal { target } => {
                if board.is_shielded(seat) || board.is_shielded(target) {
                    return;
                }
                board.record_card_interaction(seat);
                let stolen = board.players[target];
                let given = board.players[seat];
                board.players.swap(seat, target);
                let target_nick = ctx.nick(target);
                ctx.record(irc_format!(
                    "🗡️ {actor_label} ({}) stole {} from {} (giving {} {}).",
                    actor_nick.bold(),
                    stolen.display_for_peek(),
                    target_nick.as_str().bold(),
                    target_nick.as_str().bold(),
                    given.display_for_peek(),
                ));
            }
            RogueDecision::DitchToCenter { target } => {
                if board.is_shielded(target) {
                    return;
                }
                board.record_card_interaction(seat);
                let old_player_card = board.players[target];
                let old_center_card = board.center[0];
                std::mem::swap(&mut board.players[target], &mut board.center[0]);
                ctx.record(irc_format!(
                    "🗡️ {actor_label} ({}) swapped {}'s {} with Center #1 ({}).",
                    actor_nick.bold(),
                    ctx.nick(target).bold(),
                    old_player_card.display_for_peek(),
                    old_center_card.display_for_peek(),
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "rogue_test.rs"]
mod test;
