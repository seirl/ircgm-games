use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct Revealer;
pub static REVEALER: Revealer = Revealer;

impl Role for Revealer {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "revealer",
            "Revealer",
            GameSet::Daybreak,
            Team::Village,
            "Flip another player's card face up for Dawn—unless it is Evil or Independent, in which case you flip it back face down!",
        )
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        format!(
            "Flip another player's card face up for Dawn—unless it is {}, in which case you flip it back face down!",
            roster.describe_non_village_roles()
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(192, Self::prompt).mutates())
    }
}

impl Revealer {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let prompt = format!(
                "🔦 Revealer: Reply `<nick>` to flip their card, or `pass`. Players: {}",
                roster.format_players_marking_shields(shielded)
            );
            let ans = ctx
                .ask(prompt, move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept(None);
                    }
                    match roster.resolve_nick(&m.text) {
                        Some(idx) if idx == seat => {
                            AskMatch::reject("Pick ANOTHER player, not yourself!".yellow())
                        }
                        Some(idx) if shielded.contains(&idx) => {
                            AskMatch::reject("🛡️ Shielded cards cannot be flipped!".yellow())
                        }
                        Some(idx) => AskMatch::accept(Some(idx)),
                        None => AskMatch::reject("Unknown player nickname (or `pass`).".yellow()),
                    }
                })
                .await?;

            Ok(SeatEffect::new(move |label, board, rctx| {
                Self::resolve(seat, label, ans, board, rctx);
            }))
        })
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<Option<usize>>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Answer::Ok(Some(target)) = ans {
            if board.is_shielded(target) {
                return;
            }
            board.record_card_interaction(seat);
            let card = board.players[target];
            let face_role = card.face_role();
            let actor_nick = ctx.nick(seat);
            let target_nick = ctx.nick(target);
            if face_role.team() != Team::Village {
                ctx.dm(
                    seat,
                    irc_format!(
                        "🔦 {} is {} — you quickly flip it BACK face down!",
                        target_nick.as_str().bold(),
                        card.display_for_peek(),
                    ),
                );
                ctx.record(irc_format!(
                    "🔦 {actor_label} ({}) peeked {} ({}) and flipped it back face-down.",
                    actor_nick.bold(),
                    target_nick.bold(),
                    card.display_for_peek(),
                ));
            } else {
                board.face_up_players.insert(target);
                ctx.dm(
                    seat,
                    irc_format!(
                        "🔦 {} is {} — left FACE UP for everyone to see at Dawn!",
                        target_nick.as_str().bold(),
                        card.display_for_peek(),
                    ),
                );
                ctx.record(irc_format!(
                    "🔦 {actor_label} ({}) flipped {}'s card ({}) face-up!",
                    actor_nick.bold(),
                    target_nick.bold(),
                    card.display_for_peek(),
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "revealer_test.rs"]
mod test;
