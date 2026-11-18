use ircgm::prelude::*;

use crate::board::{BoardMut, CenterSlot, Roster};
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
            "Wake to view 1 center card.",
        )
        .aliases(&["aseer", "appseer"])
    }

    fn steps(&self) -> &'static [WakeStep] {
        const {
            &[
                WakeStep::solo(&APPRENTICE_SEER, |s, ctx| Box::pin(Self::prompt(s, ctx))),
            ]
        }
    }
}

impl ApprenticeSeer {
    async fn prompt(seat: usize, mut ctx: PromptCtx<'_>) -> Result<SeatEffect> {
        let roster = ctx.roster;
        let choice_hint = roster.center_choice_hint();
        let or_pass_hint = roster.center_or_pass_hint();
        let ans = ctx
            .ask(
                irc_format!(
                    "🔮 Apprentice Seer: Reply {} to view a center card, or {}:",
                    choice_hint.bold(),
                    "pass".bold(),
                ),
                move |m| {
                    if Roster::is_pass(&m.text) {
                        return AskMatch::accept_say(None, "🔮 Passed on viewing a center card.");
                    }
                    match roster.parse_center_slot(&m.text) {
                        Some(slot) => AskMatch::accept_say(
                            Some(slot),
                            format!(
                                "🔮 Locked in {} — card reveals when the phase ends!",
                                slot.label(roster)
                            ),
                        ),
                        None => AskMatch::reject(format!("Reply {or_pass_hint}.").yellow()),
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
        ans: Answer<Option<CenterSlot>>,
        board: &mut BoardMut<'_>,
        ctx: &mut ResolveCtx<'_>,
    ) {
        if let Answer::Ok(Some(slot)) = ans {
            let seen = board.peek_center(seat, slot);
            let label = slot.label(ctx.roster);
            ctx.dm(
                seat,
                irc_format!("🔮 {label} is: {}", seen.display_for_peek()),
            );
            ctx.record(irc_format!(
                "🔮 {actor_label} ({}) viewed {label} ({}).",
                ctx.nick(seat).bold(),
                seen.display_for_peek(),
            ));
        }
    }
}

#[cfg(test)]
#[path = "apprentice_seer_test.rs"]
mod test;
