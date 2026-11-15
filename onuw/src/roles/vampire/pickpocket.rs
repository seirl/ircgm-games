use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct Pickpocket;
pub static PICKPOCKET: Pickpocket = Pickpocket;

impl Role for Pickpocket {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "pickpocket",
            "Pickpocket",
            GameSet::Vampire,
            Team::Village,
            "Swap your Mark with another player's Mark at the end of Dusk and view your new Mark!",
        )
    }

    fn summary_for_deck(&self, roster: &Roster) -> String {
        if roster.deck_has_marks() {
            self.info().summary.to_owned()
        } else {
            "No Mark-placing roles are in this deck — you have nothing to steal tonight."
                .to_owned()
        }
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(60, Self::prompt).dusk().mutates())
    }
}

impl Pickpocket {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let roster = ctx.roster;
            let prompt = format!(
                "🤏 Pickpocket: Reply `<nick>` to swap Marks and view your new Mark, or `pass`. Players: {}",
                roster.format_players()
            );
            let ans = ctx
                .ask(prompt, move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept(None);
                    }
                    match roster.resolve_nick(&m.text) {
                        Some(idx) if idx == seat => AskMatch::reject("Pick ANOTHER player!".yellow()),
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
            let old_self_mark = board.marks[seat];
            board.marks.swap(seat, target);
            let new_mark = board.marks[seat];
            let target_nick = ctx.nick(target);
            ctx.dm(
                seat,
                irc_format!(
                    "🤏 Swapped Marks with {} — your new Mark is {}!",
                    target_nick.as_str().bold(),
                    new_mark.styled(),
                ),
            );
            ctx.record(irc_format!(
                "🤏 {actor_label} ({}) stole {} from {} (giving {} {}).",
                ctx.nick(seat).bold(),
                new_mark.styled(),
                target_nick.as_str().bold(),
                target_nick.as_str().bold(),
                old_self_mark.styled(),
            ));
        }
    }
}
