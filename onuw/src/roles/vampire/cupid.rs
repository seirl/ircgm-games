use ircgm::prelude::*;

use crate::board::{BoardAccess, BoardMut, Roster};
use crate::marks::Mark;
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct Cupid;
pub static CUPID: Cupid = Cupid;

impl Role for Cupid {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "cupid",
            "Cupid",
            GameSet::Vampire,
            Team::Village,
            "Wake in Dusk to give 2 players the Mark of Love (if one Lover dies in the vote, the other dies of heartbreak!).",
        )
        .places_marks()
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[WakeStep::solo(&CUPID, 35, |s, ctx| Box::pin(Self::prompt(s, ctx)))
                .dusk()
                .writes(BoardAccess::MARKS)]
        }
    }
}

impl Cupid {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let players_str = ctx.format_visible_players();
        let view = ctx.view();
        let prompt = irc_format!(
            "💘 Cupid: Reply with TWO players ({}) to bestow the Mark of Love, or {}. Players: {players_str}",
            "<nick1> <nick2>".bold(),
            "pass".bold(),
        );
        let ans = ctx
            .ask(prompt, move |m| {
                if Roster::is_pass(&m.text) {
                    return AskMatch::accept_say(None, "💘 Passed — bestowed no Marks of Love.");
                }
                let parts: Vec<&str> = m.text.split_whitespace().collect();
                if parts.len() != 2 {
                    return AskMatch::reject("Usage: <nick1> <nick2> or pass.".yellow());
                }
                let (Some(a), Some(b)) =
                    (view.resolve_nick(parts[0]), view.resolve_nick(parts[1]))
                else {
                    return AskMatch::reject("Unknown player nickname.".yellow());
                };
                if a == b {
                    return AskMatch::reject("Pick two DIFFERENT players!".yellow());
                }
                AskMatch::accept_say(
                    Some((a, b)),
                    irc_format!(
                        "💘 Linked {} and {} with the {}!",
                        view.roster.seats[a].user.nick().bold(),
                        view.roster.seats[b].user.nick().bold(),
                        Mark::Love.styled(),
                    ),
                )
            })
            .await?;

        Ok(SeatEffect::new(move |label, board, rctx| {
            Self::resolve(seat, label, ans, board, rctx);
        }))
    }

    fn resolve(
        seat: usize,
        actor_label: &str,
        ans: Answer<Option<(usize, usize)>>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Answer::Ok(Some((a, b))) = ans {
            board.set_mark(a, Mark::Love);
            board.set_mark(b, Mark::Love);
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
