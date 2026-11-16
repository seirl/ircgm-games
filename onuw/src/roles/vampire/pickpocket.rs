use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

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

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&PICKPOCKET, 166, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .writes(BoardAccess::MARKS)]
        }
    }
}

impl Pickpocket {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let players_str = ctx.format_visible_players();
        let prompt = irc_format!(
            "🤏 Pickpocket: Reply {} to swap Marks and view your new Mark, or {}. Players: {players_str}",
            "<nick>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "🤏 Passed — kept your own Mark.");
                }
                match roster.resolve_nick(&m.text) {
                    Some(idx) if idx == seat => AskMatch::reject("Pick ANOTHER player!".yellow()),
                    Some(idx) => AskMatch::accept_say(
                        Some(idx),
                        irc_format!(
                            "🤏 Locked in swapping Marks with {} — reveals when the phase ends!",
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
            let (old_self_mark, new_mark) = board.swap_marks(seat, target);
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
