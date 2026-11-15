use ircgm::prelude::*;

use crate::board::BoardCards;
use crate::marks::Mark;
use crate::role::{
    BoxFuture, DeathCtx, DeathEffect, GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, RoleTag,
    SeatEffect, Team, Wake,
};

pub struct Diseased;
pub static DISEASED: Diseased = Diseased;

impl Role for Diseased {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "diseased",
            "Diseased",
            GameSet::Vampire,
            Team::Village,
            "Give another player the Mark of Disease; anyone voting for a marked player (or sitting beside a killed Diseased) cannot win!",
        )
        .tags(&[RoleTag::PlacesMarks])
    }

    fn wake(&self) -> Option<Wake> {
        Some(Wake::solo(38, Self::prompt).dusk().mutates())
    }

    fn on_death(&self, dead_seat: usize, ctx: &DeathCtx<'_>) -> Option<DeathEffect> {
        let (l, r) = ctx.roster.neighbors(dead_seat);
        Some(DeathEffect::ForfeitWins(vec![l, r]))
    }
}

impl Diseased {
    fn prompt<'a>(seat: usize, mut ctx: PromptCtx<'a>) -> BoxFuture<'a, Result<SeatEffect>> {
        Box::pin(async move {
            let roster = ctx.roster;
            let prompt = format!(
                "🤢 Diseased: Reply `<nick>` to place the Mark of Disease on another player! Players: {}",
                roster.format_players()
            );
            let ans = ctx
                .ask(prompt, move |m| match roster.resolve_nick(&m.text) {
                    Some(idx) if idx == seat => AskMatch::reject("Pick ANOTHER player!".yellow()),
                    Some(idx) => AskMatch::accept_say(
                        idx,
                        irc_format!(
                            "🤢 Placed {} on {}!",
                            Mark::Disease.styled(),
                            roster.seats[idx].user.nick().bold(),
                        ),
                    ),
                    None => AskMatch::reject("Unknown player nickname.".yellow()),
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
        ans: Answer<usize>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let target = match ans {
            Answer::Ok(idx) => idx,
            Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                ctx.roster.neighbors(seat).0
            }
        };
        board.marks[target] = Mark::Disease;
        ctx.record(irc_format!(
            "🤢 {actor_label} ({}) placed {} on {}.",
            ctx.nick(seat).bold(),
            Mark::Disease.styled(),
            ctx.nick(target).bold(),
        ));
    }
}
