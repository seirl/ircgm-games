use ircgm::prelude::*;

use crate::board::{BoardCards, Roster};
use crate::role::{
    BoxFuture, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, Wake,
};

pub struct Thing;
pub static THING: Thing = Thing;

impl Role for Thing {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "thing",
            "Thing",
            GameSet::Vampire,
            Team::Village,
            "Go bump in the night! May tap either your immediate left or right neighbor on the shoulder so they know Thing sits beside them.",
        )
        .aliases(&["the thing"])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(141, Self::prompt))
    }
}

impl Thing {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let roster = ctx.roster;
            let (left, right) = roster.neighbors(seat);
            let prompt = irc_format!(
                "👻 Thing: Reply {} ({}), {} ({}), or {} to go bump in the night!",
                "left".bold(),
                ctx.nick(left),
                "right".bold(),
                ctx.nick(right),
                "pass".bold(),
            );
            let ans = ctx
                .ask(prompt, move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept(None);
                    }
                    match m.text.trim().to_ascii_lowercase().as_str() {
                        "left" | "l" => AskMatch::accept_say(
                            Some(left),
                            format!("👻 Bumped {}!", roster.seats[left].user.nick()),
                        ),
                        "right" | "r" => AskMatch::accept_say(
                            Some(right),
                            format!("👻 Bumped {}!", roster.seats[right].user.nick()),
                        ),
                        other => match roster.resolve_nick(other) {
                            Some(idx) if idx == left || idx == right => AskMatch::accept_say(
                                Some(idx),
                                format!("👻 Bumped {}!", roster.seats[idx].user.nick()),
                            ),
                            _ => AskMatch::reject(
                                "Pick left, right (or your neighbor's nickname), or pass!".yellow(),
                            ),
                        },
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
        _board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Answer::Ok(Some(target)) = ans {
            let (left, _) = ctx.roster.neighbors(seat);
            let shoulder = if target == left { "right" } else { "left" };
            let side = if target == left { "right" } else { "left" };
            ctx.dm(
                target,
                irc_format!(
                    "{} Something tapped your {shoulder} shoulder — your immediate {side} neighbor is Thing!",
                    "👻 *BUMP!*".bold().yellow(),
                ),
            );
            ctx.record(irc_format!(
                "👻 {actor_label} ({}) bumped neighbor {}.",
                ctx.nick(seat).bold(),
                ctx.nick(target).bold(),
            ));
        } else {
            ctx.record(irc_format!(
                "👻 {actor_label} ({}) stayed quiet.",
                ctx.nick(seat).bold(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "thing_test.rs"]
mod test;
