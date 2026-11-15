use ircgm::prelude::*;

use crate::board::BoardCards;
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
            "Go bump in the night! Tap either your immediate left or right neighbor in DM so they know Thing sits beside them.",
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
                "👻 Thing: Reply {} ({}) or {} ({}) to go bump in the night!",
                "left".bold(),
                ctx.nick(left),
                "right".bold(),
                ctx.nick(right),
            );
            let ans = ctx
                .ask(
                    prompt,
                    move |m| match m.text.trim().to_ascii_lowercase().as_str() {
                        "left" | "l" => AskMatch::accept_say(
                            left,
                            format!("👻 Bumped {}!", roster.seats[left].user.nick()),
                        ),
                        "right" | "r" => AskMatch::accept_say(
                            right,
                            format!("👻 Bumped {}!", roster.seats[right].user.nick()),
                        ),
                        other => match roster.resolve_nick(other) {
                            Some(idx) if idx == left || idx == right => AskMatch::accept_say(
                                idx,
                                format!("👻 Bumped {}!", roster.seats[idx].user.nick()),
                            ),
                            _ => AskMatch::reject(
                                "Pick left or right (or your neighbor's nickname)!".yellow(),
                            ),
                        },
                    },
                )
                .await?;

            Ok(SeatEffect::new(move |label, board, rctx| {
                Self::resolve(seat, label, ans, board, rctx);
            }))
        })
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<usize>,
        _board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let (left, _) = ctx.roster.neighbors(seat);
        let target = ans.ok().unwrap_or(left);
        ctx.dm(
            target,
            irc_format!(
                "{} Something went bump in the night right beside you (an immediate neighbor is Thing)!",
                "👻 *BUMP!*".bold().yellow(),
            ),
        );
        ctx.record(irc_format!(
            "👻 {actor_label} ({}) bumped neighbor {}.",
            ctx.nick(seat).bold(),
            ctx.nick(target).bold(),
        ));
    }
}

#[cfg(test)]
#[path = "thing_test.rs"]
mod test;
