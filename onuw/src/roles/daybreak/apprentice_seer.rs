use ircgm::prelude::*;

use crate::board::{BoardMut, Roster};
use crate::role::{GameSet, PromptCtx, ResolveCtx, Role, RoleInfo, SeatEffect, Team, WakeStep};

pub struct ApprenticeSeer;
pub static APPRENTICE_SEER: ApprenticeSeer = ApprenticeSeer;

impl Role for ApprenticeSeer {
    fn info(&self) -> RoleInfo {
        RoleInfo::new(
            "apprentice_seer",
            "Apprentice Seer",
            GameSet::Daybreak,
            Team::Village,
            "Wake to view 1 of the 3 center cards.",
        )
        .aliases(&["aseer", "appseer"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&APPRENTICE_SEER, 152, |s, ctx| Box::pin(Self::prompt(s, ctx))),
            ]
        }
    }
}

impl ApprenticeSeer {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let ans = ctx
            .ask(
                irc_format!(
                    "🔮 Apprentice Seer: Reply {}, {}, or {} to view a center card, or {}:",
                    "1".bold(),
                    "2".bold(),
                    "3".bold(),
                    "pass".bold(),
                ),
                |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept_say(None, "🔮 Passed on viewing a center card.");
                    }
                    match Roster::parse_center_slot(&m.text) {
                        Some(c_idx) => AskMatch::accept_say(
                            Some(c_idx),
                            format!(
                                "🔮 Locked in Center #{} — card reveals when the phase ends!",
                                c_idx + 1
                            ),
                        ),
                        None => AskMatch::reject("Reply 1, 2, 3, or pass.".yellow()),
                    }
                },
            )
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
        if let Answer::Ok(Some(c_idx)) = ans {
            let seen = board.peek_center(seat, c_idx);
            ctx.dm(
                seat,
                irc_format!("🔮 Center #{} is: {}", c_idx + 1, seen.display_for_peek()),
            );
            ctx.record(irc_format!(
                "🔮 {actor_label} ({}) viewed Center #{} ({}).",
                ctx.nick(seat).bold(),
                c_idx + 1,
                seen.display_for_peek(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "apprentice_seer_test.rs"]
mod test;
