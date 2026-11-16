use ircgm::prelude::*;

use crate::board::BoardCards;
use crate::marks::Mark;
use crate::role::{
    GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, RoleTag, SeatEffect, Team, Wake,
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
            "Place the Mark of Disease on your left or right neighbor in Dusk; anyone who votes for you or the marked player cannot win!",
        )
        .tags(&[RoleTag::PlacesMarks])
    }

    fn wake(&self) -> Option<Wake> {
        Some(
            Wake::solo(34, |seat, ctx| Box::pin(Self::prompt(seat, ctx)))
                .dusk()
                .mutates(),
        )
    }
}

impl Diseased {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let (left, right) = roster.neighbors(seat);
        let prompt = irc_format!(
            "🤢 Diseased: You MUST place the Mark of Disease on your {} ({}) or {} ({}) neighbor!",
            "left".bold(),
            ctx.nick(left),
            "right".bold(),
            ctx.nick(right),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                let raw = m.text.trim();
                let chosen = match raw.to_ascii_lowercase().as_str() {
                    "left" | "l" => Some(left),
                    "right" | "r" => Some(right),
                    _ => roster
                        .resolve_nick(raw)
                        .filter(|&idx| idx == left || idx == right),
                };
                match chosen {
                    Some(idx) => AskMatch::accept_say(
                        idx,
                        irc_format!(
                            "🤢 Placed {} on {}!",
                            Mark::Disease.styled(),
                            roster.seats[idx].user.nick().bold(),
                        ),
                    ),
                    None => AskMatch::reject(
                        "You MUST pick your immediate left or right neighbor!".yellow(),
                    ),
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
        ans: Answer<usize>,
        board: &mut BoardCards,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let target = match ans {
            Answer::Ok(idx) => idx,
            Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                let (left, right) = ctx.roster.neighbors(seat);
                if fastrand::bool() { left } else { right }
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
