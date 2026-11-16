use ircgm::prelude::*;

use crate::board::{BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

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

    fn steps(&self) -> &'static [WakeStep] {
        const { &[WakeStep::solo(&THING, 141, |s, ctx| Box::pin(Self::prompt(s, ctx)))] }
    }
}

impl Thing {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
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
                    return AskMatch::accept_say(None, "👻 Passed — stayed quiet tonight.");
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
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<Option<usize>>,
        _board: &mut BoardMut<'_>,
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
