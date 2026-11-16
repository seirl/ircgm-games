use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::marks::Mark;
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, SeatEffect, Team, Wake,
};

pub struct Instigator;
pub static INSTIGATOR: Instigator = Instigator;

impl Role for Instigator {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "instigator",
            "Instigator",
            GameSet::Vampire,
            Team::Village,
            "Wake in Dusk to optionally give ANY player (including yourself) the Mark of the Traitor, flipping their Village/Evil team!",
        )
        .tags(&[RoleTag::PlacesMarks])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(38, Self::prompt).dusk().mutates())
    }
}

impl Instigator {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let roster = ctx.roster;
            let prompt = irc_format!(
                "🐍 Instigator: Reply {} to bestow the Mark of the Traitor (including yourself), or {}. Players: {}",
                "<nick>".bold(),
                "pass".bold(),
                roster.format_players(),
            );
            let ans = ctx
                .ask(prompt, move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept_say(
                            None,
                            "🐍 Passed — bestowed no Mark of the Traitor.",
                        );
                    }
                    match roster.resolve_nick(&m.text) {
                        Some(idx) => AskMatch::accept_say(
                            Some(idx),
                            irc_format!(
                                "🐍 Bestowed {} on {}!",
                                Mark::Traitor.styled(),
                                roster.seats[idx].user.nick().bold(),
                            ),
                        ),
                        None => AskMatch::reject("Unknown player nickname (or pass).".yellow()),
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
            board.marks[target] = Mark::Traitor;
            ctx.record(irc_format!(
                "🐍 {actor_label} ({}) placed {} on {}.",
                ctx.nick(seat).bold(),
                Mark::Traitor.styled(),
                ctx.nick(target).bold(),
            ));
        }
    }
}
