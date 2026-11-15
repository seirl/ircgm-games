use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct Robber;
pub static ROBBER: Robber = Robber;

impl Role for Robber {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "robber",
            "Robber",
            GameSet::Base,
            Team::Village,
            "Wake to optionally swap your card with another player's card and view your new card.",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(160, Self::prompt).mutates())
    }
}

impl Robber {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let prompt = format!(
                "🦹 Robber: Reply with `<nick>` to steal their card, or `pass`. Players: {}",
                roster.format_players_marking_shields(shielded)
            );
            let ans = ctx
                .ask(prompt, move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept(None);
                    }
                    match roster.resolve_nick(&m.text) {
                        Some(idx) if idx == seat => AskMatch::reject(
                            "Pick ANOTHER player (or `pass`), not yourself!".yellow(),
                        ),
                        Some(idx) if shielded.contains(&idx) => AskMatch::reject(
                            "🛡️ That player's card is shielded by the Sentinel!".yellow(),
                        ),
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
        let actor_nick = ctx.nick(seat);
        if let Answer::Ok(Some(target_seat)) = ans {
            if board.is_shielded(target_seat) || board.is_shielded(seat) {
                ctx.dm(seat, "🛡️ Robber swap blocked by a Sentinel shield!".yellow());
                return;
            }
            board.record_card_interaction(seat);
            board.players.swap(seat, target_seat);
            let stolen = board.players[seat];
            let given = board.players[target_seat];
            let victim_nick = ctx.nick(target_seat);
            ctx.dm(
                seat,
                irc_format!(
                    "🦹 You robbed {} and got: {}!",
                    victim_nick.as_str().bold(),
                    stolen.display_for_peek(),
                ),
            );
            ctx.record(irc_format!(
                "🦹 {actor_label} ({}) stole {} from {} (giving {} {}).",
                actor_nick.bold(),
                stolen.display_for_peek(),
                victim_nick.as_str().bold(),
                victim_nick.as_str().bold(),
                given.display_for_peek(),
            ));
        } else {
            ctx.record(irc_format!(
                "🦹 {actor_label} ({}) did not rob anyone.",
                actor_nick.bold(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "robber_test.rs"]
mod test;
