use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct Sentinel;
pub static SENTINEL: Sentinel = Sentinel;

impl Role for Sentinel {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "sentinel",
            "Sentinel",
            GameSet::Daybreak,
            Team::Village,
            "Wake before Night (`Dusk`) to optionally place a Shield token on another player's card, preventing it from being viewed or moved.",
        )
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(2, Self::prompt).before_copycat().mutates())
    }
}

impl Sentinel {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let roster = ctx.roster;
            let prompt = format!(
                "🛡️ Sentinel: Reply with `<nick>` to place a Shield token on another player's card, or `pass`. Players: {}",
                roster.format_players()
            );
            let ans = ctx
                .ask(prompt, move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept(None);
                    }
                    match roster.resolve_nick(&m.text) {
                        Some(idx) if idx == seat => AskMatch::reject(
                            "You cannot shield your own card! Pick another player or `pass`."
                                .yellow(),
                        ),
                        Some(idx) => AskMatch::accept_say(
                            Some(idx),
                            irc_format!(
                                "🛡️ Placed a Shield token on {}'s card!",
                                roster.seats[idx].user.nick().bold(),
                            ),
                        ),
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
        if let Answer::Ok(Some(target)) = ans {
            board.shielded.insert(target);
            ctx.record(irc_format!(
                "🛡️ {actor_label} ({}) placed a Shield token on {}'s card.",
                actor_nick.bold(),
                ctx.nick(target).bold(),
            ));
        } else {
            ctx.record(irc_format!(
                "🛡️ {actor_label} ({}) placed no Shield token.",
                actor_nick.bold(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "sentinel_test.rs"]
mod test;
