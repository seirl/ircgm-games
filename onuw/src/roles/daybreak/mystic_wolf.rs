use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, SeatEffect, Team, Wake,
};

pub struct MysticWolf;
pub static MYSTIC_WOLF: MysticWolf = MysticWolf;

impl Role for MysticWolf {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "mystic_wolf",
            "Mystic Wolf",
            GameSet::Daybreak,
            Team::Werewolf,
            "Wake with the Werewolves, then look at 1 other player's card!",
        )
        .aliases(&["mw", "mystic"])
        .tags(&[RoleTag::WolfPack])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(123, Self::prompt))
    }
}

impl MysticWolf {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let (roster, shielded) = (ctx.roster, ctx.shielded);
            let prompt = format!(
                "🐺🔮 Mystic Wolf: Reply `<nick>` to inspect another player's card, or `pass`. Players: {}",
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
        if let Answer::Ok(Some(target)) = ans {
            if board.is_shielded(target) {
                return;
            }
            board.record_card_interaction(seat);
            let card = board.players[target];
            let target_nick = ctx.nick(target);
            ctx.dm(
                seat,
                irc_format!(
                    "🐺🔮 {}'s card is: {}",
                    target_nick.as_str().bold(),
                    card.display_for_peek()
                ),
            );
            ctx.record(irc_format!(
                "🐺🔮 {actor_label} ({}) viewed {}'s card ({}).",
                ctx.nick(seat).bold(),
                target_nick.bold(),
                card.display_for_peek(),
            ));
        }
    }
}
