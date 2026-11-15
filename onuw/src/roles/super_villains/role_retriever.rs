use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct RoleRetriever;
pub static ROLE_RETRIEVER: RoleRetriever = RoleRetriever;

impl Role for RoleRetriever {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "role_retriever",
            "Role Retriever",
            GameSet::SuperVillains,
            Team::Village,
            "May swap your card with another player's card—and BOTH of you see the new cards!",
        )
        .aliases(&["rr", "retriever"])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(162, Self::prompt).mutates())
    }
}

impl RoleRetriever {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let prompt = format!(
                "🐕 Role Retriever: Reply `<nick>` to swap cards with another player (they will see their new card!), or `pass`. Players: {}",
                roster.format_players_marking_shields(shielded)
            );
            let ans = ctx
                .ask(prompt, move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept(None);
                    }
                    match roster.resolve_nick(&m.text) {
                        Some(idx) if idx == seat => {
                            AskMatch::reject("Pick ANOTHER player (or `pass`)!".yellow())
                        }
                        Some(idx) if shielded.contains(&idx) => {
                            AskMatch::reject("🛡️ Shielded players cannot be swapped!".yellow())
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
            if board.is_shielded(seat) || board.is_shielded(target) {
                return;
            }
            board.record_card_interaction(seat);
            board.players.swap(seat, target);
            let actor_new = board.players[seat];
            let victim_new = board.players[target];
            let victim_nick = ctx.nick(target);

            ctx.dm(
                seat,
                irc_format!(
                    "🐕 Swapped with {} — you fetched {}!",
                    victim_nick.as_str().bold(),
                    actor_new.display_for_peek(),
                ),
            );
            ctx.dm(
                target,
                irc_format!(
                    "🐕 The Role Retriever swapped your card! Your card is now {}!",
                    victim_new.display_for_peek(),
                ),
            );
            ctx.record(irc_format!(
                "🐕 {actor_label} ({}) fetched {} from {} (giving {} {}).",
                ctx.nick(seat).bold(),
                actor_new.display_for_peek(),
                victim_nick.as_str().bold(),
                victim_nick.as_str().bold(),
                victim_new.display_for_peek(),
            ));
        }
    }
}
