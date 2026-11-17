use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::marks::Mark;
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

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
        .places_marks()
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&INSTIGATOR, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .dusk()
                .writes(BoardAccess::MARKS)]
        }
    }
}

impl Instigator {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let players_str = ctx.roster.format_players();
        let prompt = irc_format!(
            "🐍 Instigator: Reply {} to bestow the Mark of the Traitor (including yourself), or {}. Players: {players_str}",
            "<nick>".bold(),
            "pass".bold(),
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
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<Option<usize>>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Answer::Ok(Some(target)) = ans {
            board.set_mark(target, Mark::Traitor);
            ctx.record(irc_format!(
                "🐍 {actor_label} ({}) placed {} on {}.",
                ctx.nick(seat).bold(),
                Mark::Traitor.styled(),
                ctx.nick(target).bold(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "instigator_test.rs"]
mod test;
