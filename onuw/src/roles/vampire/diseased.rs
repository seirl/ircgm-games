use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut};
use crate::marks::Mark;
use crate::role::{
    GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep, WinnerFilterCtx,
    WinnerMap,
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
        .places_marks()
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&DISEASED, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .dusk()
                .writes(BoardAccess::MARKS)]
        }
    }

    fn apply_forfeits(
        &self,
        holder: usize,
        ctx: &mut WinnerFilterCtx<'_>,
        winners: &mut WinnerMap,
    ) {
        for voter in 0..ctx.roster.len() {
            if ctx.votes[voter] == holder {
                winners.remove(&voter);
            }
        }
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
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        let target = match ans {
            Answer::Ok(idx) => idx,
            Answer::TimedOut | Answer::Left { .. } | Answer::Unreachable { .. } => {
                let (left, right) = ctx.roster.neighbors(seat);
                if fastrand::bool() { left } else { right }
            }
        };
        board.set_mark(target, Mark::Disease);
        ctx.record(irc_format!(
            "🤢 {actor_label} ({}) placed {} on {}.",
            ctx.nick(seat).bold(),
            Mark::Disease.styled(),
            ctx.nick(target).bold(),
        ));
    }
}

#[cfg(test)]
#[path = "diseased_test.rs"]
mod test;
