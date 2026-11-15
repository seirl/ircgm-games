use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::marks::Mark;
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, SeatEffect, Team, Wake,
};

pub struct Cupid;
pub static CUPID: Cupid = Cupid;

impl Role for Cupid {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "cupid",
            "Cupid",
            GameSet::Vampire,
            Team::Village,
            "Wake in Dusk to give 2 players the Mark of Love (if one Lover dies at Dawn, the other dies of heartbreak!).",
        )
        .tags(&[RoleTag::PlacesMarks])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(35, Self::prompt).dusk().mutates())
    }
}

impl Cupid {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let roster = ctx.roster;
            let prompt = irc_format!(
                "💘 Cupid: Reply with TWO players ({}) to bestow the Mark of Love, or {}. Players: {}",
                "<nick1> <nick2>".bold(),
                "pass".bold(),
                roster.format_players(),
            );
            let ans = ctx
                .ask(prompt, move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept(None);
                    }
                    let parts: Vec<&str> = m.text.split_whitespace().collect();
                    if parts.len() != 2 {
                        return AskMatch::reject("Usage: <nick1> <nick2> or pass.".yellow());
                    }
                    let (Some(a), Some(b)) = (
                        roster.resolve_nick(parts[0]),
                        roster.resolve_nick(parts[1]),
                    ) else {
                        return AskMatch::reject("Unknown player nickname.".yellow());
                    };
                    if a == b {
                        return AskMatch::reject("Pick two DIFFERENT players!".yellow());
                    }
                    AskMatch::accept_say(
                        Some((a, b)),
                        irc_format!(
                            "💘 Linked {} and {} with the {}!",
                            roster.seats[a].user.nick().bold(),
                            roster.seats[b].user.nick().bold(),
                            Mark::Love.styled(),
                        ),
                    )
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
        ans: Answer<Option<(usize, usize)>>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Answer::Ok(Some((a, b))) = ans {
            board.marks[a] = Mark::Love;
            board.marks[b] = Mark::Love;
            ctx.record(irc_format!(
                "💘 {actor_label} ({}) linked {} and {} with {}.",
                ctx.nick(seat).bold(),
                ctx.nick(a).bold(),
                ctx.nick(b).bold(),
                Mark::Love.styled(),
            ));
        }
    }
}
