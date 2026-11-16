use ircgm::prelude::*;

use crate::board::Roster;
use crate::role::{
    BoxFuture, GameSet, PromptCtx, Role, RoleInfo, SeatEffect, SpecialVictory, Team, VictoryCtx,
    Wake,
};

pub struct Mortician;
pub static MORTICIAN: Mortician = Mortician;

impl Role for Mortician {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "mortician",
            "Mortician",
            GameSet::Alien,
            Team::Independent,
            "May peek at your left neighbor's card, right neighbor's card, or your own card! Win ONLY if you survive AND at least one neighbor dies!",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(197, Self::prompt).follow_on_copy())
    }

    fn special_victory(&self, seat: usize, ctx: &VictoryCtx<'_>) -> Option<SpecialVictory> {
        if ctx.is_dead(seat) {
            return None;
        }
        let (left, right) = ctx.roster.neighbors(seat);
        (ctx.is_dead(left) || ctx.is_dead(right)).then_some(SpecialVictory::independent())
    }
}

impl Mortician {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let (left, right) = roster.neighbors(seat);
            let candidates: Vec<usize> = [left, right, seat]
                .into_iter()
                .filter(|s| !shielded.contains(s))
                .collect();

            let chosen = if candidates.is_empty() {
                ctx.say("⚰️ Mortician: Your card and both neighbors' cards are shielded — you cannot peek tonight!");
                None
            } else {
                let left_nick = roster.seats[left].user.nick();
                let right_nick = roster.seats[right].user.nick();
                let prompt = irc_format!(
                    "⚰️ Mortician: Reply {} ({}), {} ({}), {}, or {} to peek at one card!",
                    "left".bold(),
                    left_nick,
                    "right".bold(),
                    right_nick,
                    "self".bold(),
                    "pass".bold(),
                );
                let ans = ctx
                    .ask(prompt, move |m| {
                        let raw = m.text.trim();
                        if Roster::is_pass(raw) {
                            return AskMatch::accept_say(
                                None,
                                "⚰️ Passed — not peeking at any card.",
                            );
                        }
                        let lower = raw.to_ascii_lowercase();
                        let target = match lower.as_str() {
                            "left" | "l" => Some(left),
                            "right" | "r" => Some(right),
                            "self" | "me" | "own" => Some(seat),
                            _ => roster.resolve_nick(raw).filter(|&i| i == left || i == right || i == seat),
                        };
                        match target {
                            Some(t) if shielded.contains(&t) => {
                                AskMatch::reject("🛡️ That card is shielded!".yellow())
                            }
                            Some(t) if t == seat => AskMatch::accept_say(
                                Some(t),
                                "⚰️ Locked in peeking at your own card — reveals when the phase ends!",
                            ),
                            Some(t) => AskMatch::accept_say(
                                Some(t),
                                irc_format!(
                                    "⚰️ Locked in peeking at {}'s card — reveals when the phase ends!",
                                    roster.seats[t].user.nick().bold(),
                                ),
                            ),
                            None => AskMatch::reject(
                                "Reply left, right, self, or pass.".yellow(),
                            ),
                        }
                    })
                    .await?;
                ans.ok().flatten()
            };

            Ok(SeatEffect::new(move |label, board, rctx| {
                let actor_nick = rctx.nick(seat);
                match chosen {
                    Some(target) if !board.is_shielded(target) => {
                        board.record_card_interaction(seat);
                        let seen = board.players[target];
                        if target == seat {
                            rctx.dm(
                                seat,
                                irc_format!(
                                    "⚰️ Mortician Peek: Your own card is {}!",
                                    seen.display_for_peek(),
                                ),
                            );
                            rctx.record(irc_format!(
                                "⚰️ {label} ({}) peeked at their own card ({}).",
                                actor_nick.bold(),
                                seen.display_for_peek(),
                            ));
                        } else {
                            let target_nick = rctx.nick(target);
                            rctx.dm(
                                seat,
                                irc_format!(
                                    "⚰️ Mortician Peek: {} holds {}!",
                                    target_nick.as_str().bold(),
                                    seen.display_for_peek(),
                                ),
                            );
                            rctx.record(irc_format!(
                                "⚰️ {label} ({}) peeked at {}'s card and saw {}.",
                                actor_nick.bold(),
                                target_nick.bold(),
                                seen.display_for_peek(),
                            ));
                        }
                    }
                    _ => {
                        rctx.record(irc_format!(
                            "⚰️ {label} ({}) did not peek at any card.",
                            actor_nick.bold(),
                        ));
                    }
                }
            }))
        })
    }
}
